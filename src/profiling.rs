//! Tracy client bootstrap and frame markers for the profiler build.

/// Starts the Tracy client for the current process.
pub fn start() -> profiling::tracy_client::Client {
    profiling::tracy_client::Client::start()
}

/// Emits a frame boundary when a Tracy client is active.
pub fn finish_frame() {
    profiling::finish_frame!();
}
