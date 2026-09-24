//! Asynchronous process output primitives.
//!
//! A producer appends to the buffer owned by its current thread. At the end
//! of a frame that buffer is moved into the unbounded shared queue as one
//! batch. Batches from one producer remain ordered; batches from different
//! producers are ordered by the queue lock acquisition order.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{Arguments, Write as FmtWrite};
use std::io::{self, Write};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

struct QueueState {
    batches: VecDeque<OutputBatch>,
    accepting: bool,
}

struct OutputBatch {
    text: String,
    completion: Option<SyncSender<()>>,
}

struct SharedQueue {
    state: Mutex<QueueState>,
    wake: Condvar,
}

impl SharedQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                batches: VecDeque::new(),
                accepting: true,
            }),
            wake: Condvar::new(),
        }
    }

    fn push(&self, batch: String) {
        if batch.is_empty() {
            return;
        }
        self.push_batch(OutputBatch {
            text: batch,
            completion: None,
        });
    }

    fn push_final(&self, batch: String, completion: SyncSender<()>) {
        if batch.is_empty() {
            let _ = completion.send(());
            return;
        }
        let mut state = self.state.lock().expect("output queue mutex poisoned");
        if state.accepting {
            state.batches.push_back(OutputBatch {
                text: batch,
                completion: Some(completion),
            });
            state.accepting = false;
            self.wake.notify_one();
        } else {
            let _ = completion.send(());
        }
    }

    fn push_batch(&self, batch: OutputBatch) {
        let mut state = self.state.lock().expect("output queue mutex poisoned");
        if state.accepting {
            state.batches.push_back(batch);
            self.wake.notify_one();
        }
    }

    fn close(&self) {
        let mut state = self.state.lock().expect("output queue mutex poisoned");
        state.accepting = false;
        self.wake.notify_one();
    }

    fn take_next(&self) -> Option<OutputBatch> {
        let mut state = self.state.lock().expect("output queue mutex poisoned");
        loop {
            if let Some(batch) = state.batches.pop_front() {
                return Some(batch);
            }
            if !state.accepting {
                return None;
            }
            state = self.wake.wait(state).expect("output queue mutex poisoned");
        }
    }

    #[cfg(test)]
    fn take_all(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("output queue mutex poisoned")
            .batches
            .drain(..)
            .map(|batch| batch.text)
            .collect()
    }
}

thread_local! {
    static LOCAL_OUTPUT: RefCell<Option<LocalOutput>> = const { RefCell::new(None) };
}

struct LocalOutput {
    queue: Arc<SharedQueue>,
    buffer: String,
}

/// Owns the dedicated stdout writer and its unbounded batch queue.
pub struct OutputService {
    queue: Arc<SharedQueue>,
    writer: Option<JoinHandle<()>>,
}

impl OutputService {
    pub fn start() -> Self {
        let queue = Arc::new(SharedQueue::new());
        let writer_queue = Arc::clone(&queue);
        let writer = thread::spawn(move || write_batches(writer_queue));
        Self {
            queue,
            writer: Some(writer),
        }
    }

    /// Makes `print_local!` use a buffer owned by the calling thread.
    pub fn attach_to_current_thread(&self) -> OutputScope {
        let previous = LOCAL_OUTPUT.with(|local| {
            local.replace(Some(LocalOutput {
                queue: Arc::clone(&self.queue),
                buffer: String::new(),
            }))
        });
        OutputScope { previous }
    }

    /// Publishes a non-frame message without waiting for stdout.
    pub fn submit(&self, message: String) {
        self.queue.push(message);
    }

    /// Enqueues the final asynchronous output and waits until stdout is flushed.
    /// After this call, the queue rejects every subsequent output batch.
    pub fn submit_final_and_wait(&self, message: String) {
        flush_frame();
        let (completion_sender, completion_receiver) = mpsc::sync_channel(0);
        self.queue.push_final(message, completion_sender);
        completion_receiver
            .recv()
            .expect("output writer stopped before final output was flushed");
    }
}

impl Drop for OutputService {
    fn drop(&mut self) {
        self.queue.close();
        if let Some(writer) = self.writer.take() {
            writer.join().expect("output thread panicked");
        }
    }
}

/// Restores the previous thread-local output context on drop.
pub struct OutputScope {
    previous: Option<LocalOutput>,
}

impl Drop for OutputScope {
    fn drop(&mut self) {
        flush_frame();
        LOCAL_OUTPUT.with(|local| {
            local.replace(self.previous.take());
        });
    }
}

/// Starts a new local frame buffer on the current producer thread.
pub fn begin_frame() {
    LOCAL_OUTPUT.with(|local| {
        if let Some(output) = local.borrow_mut().as_mut() {
            output.buffer.clear();
        }
    });
}

/// Moves the complete current frame buffer into the shared queue.
pub fn flush_frame() {
    let batch = LOCAL_OUTPUT.with(|local| {
        local
            .borrow_mut()
            .as_mut()
            .map(|output| std::mem::take(&mut output.buffer))
    });
    if let Some(batch) = batch {
        LOCAL_OUTPUT.with(|local| {
            if let Some(output) = local.borrow().as_ref() {
                output.queue.push(batch);
            }
        });
    }
}

/// Appends formatted output locally, adding no stdout operation on the hot path.
pub fn print_local(args: Arguments<'_>) {
    LOCAL_OUTPUT.with(|local| {
        if let Some(output) = local.borrow_mut().as_mut() {
            output
                .buffer
                .write_fmt(args)
                .expect("writing to a String cannot fail");
        }
    });
}

fn write_batches(queue: Arc<SharedQueue>) {
    let stdout = io::stdout();
    while let Some(batch) = queue.take_next() {
        let mut stdout_lock = stdout.lock();
        let _ = stdout_lock.write_all(batch.text.as_bytes());
        let _ = stdout_lock.flush();
        if let Some(completion) = batch.completion {
            let _ = completion.send(());
        }
    }
    let mut stdout_lock = stdout.lock();
    let _ = stdout_lock.flush();
}

#[macro_export]
macro_rules! print_local {
    ($($arg:tt)*) => {{
        $crate::output::print_local(format_args!("{}\n", format_args!($($arg)*)));
    }};
}

#[cfg(test)]
mod tests {
    use super::SharedQueue;

    #[test]
    fn preserves_batch_order_without_splitting_lines() {
        let queue = SharedQueue::new();

        queue.push("frame 1\nlinha 1\nlinha 2\n".to_owned());
        queue.push("frame 2\nlinha 3\n".to_owned());

        assert_eq!(
            queue.take_all(),
            vec!["frame 1\nlinha 1\nlinha 2\n", "frame 2\nlinha 3\n"]
        );
    }

    #[test]
    fn closing_keeps_queued_batches_draining_before_eof() {
        let queue = SharedQueue::new();
        queue.push("last batch\n".to_owned());
        queue.close();

        assert_eq!(
            queue.take_next().map(|batch| batch.text),
            Some("last batch\n".to_owned())
        );
        assert!(queue.take_next().is_none());
    }

    #[test]
    fn final_batch_closes_the_queue_and_rejects_later_output() {
        let queue = SharedQueue::new();
        let (sender, receiver) = std::sync::mpsc::sync_channel(0);

        queue.push_final("final overlays\n".to_owned(), sender);
        queue.push("must not be written\n".to_owned());

        let final_batch = queue.take_next().expect("final batch should be queued");
        assert_eq!(final_batch.text, "final overlays\n");
        assert!(final_batch.completion.is_some());
        assert!(queue.take_next().is_none());
        assert!(queue.state.lock().expect("queue mutex poisoned").batches.is_empty());

        drop(final_batch);
        assert!(receiver.try_recv().is_err());
    }
}
