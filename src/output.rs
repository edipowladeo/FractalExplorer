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
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

struct QueueState {
    batches: VecDeque<String>,
    accepting: bool,
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

    fn take_next(&self) -> Option<String> {
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
        let _ = stdout_lock.write_all(batch.as_bytes());
        let _ = stdout_lock.flush();
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

        assert_eq!(queue.take_next().as_deref(), Some("last batch\n"));
        assert!(queue.take_next().is_none());
    }
}
