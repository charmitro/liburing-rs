//! Async I/O support for io_uring
//!
//! This module provides async/await interfaces for io_uring operations.
//! Enable with the `async-tokio` or `async-async-std` features.
//!
//! **Note**: Only one async runtime feature should be enabled at a time.
//! If both are enabled, tokio will be used by default.

#[cfg(feature = "async-tokio")]
pub mod tokio_impl;

#[cfg(feature = "async-async-std")]
pub mod async_std_impl;

// Re-export AsyncIoUring from the appropriate runtime implementation
// If both features are enabled, prefer tokio
#[cfg(feature = "async-tokio")]
pub use tokio_impl::AsyncIoUring;

#[cfg(all(feature = "async-async-std", not(feature = "async-tokio")))]
pub use async_std_impl::AsyncIoUring;
