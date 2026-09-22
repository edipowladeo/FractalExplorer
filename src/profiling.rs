//! Optional Tracy client bootstrap and frame markers.

/// Starts the Tracy client for the current process.
#[cfg(feature = "tracy")]
pub fn start() -> Option<profiling::tracy_client::Client> {
    Some(profiling::tracy_client::Client::start())
}

#[cfg(not(feature = "tracy"))]
pub fn start() {}

pub const fn enabled() -> bool {
    cfg!(feature = "tracy")
}

/// Emits a frame boundary when a Tracy client is active.
#[cfg(feature = "tracy")]
pub fn finish_frame() {
    profiling::finish_frame!();
}

#[cfg(not(feature = "tracy"))]
pub fn finish_frame() {}

#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! profile_scope {
    ($name:literal) => {
        ::profiling::scope!($name);
    };
}

#[cfg(not(feature = "tracy"))]
#[macro_export]
macro_rules! profile_scope {
    ($name:literal) => {
        let _ = $name;
    };
}
