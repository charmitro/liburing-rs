//! async-std runtime integration for io_uring

use crate::{
    ops::{PrepareOp, SqeExt},
    IoUring, Result,
};
use std::sync::{Arc, Mutex};

/// Async io_uring instance integrated with async-std runtime
///
/// This wraps an `IoUring` instance and integrates it with async-std's async runtime,
/// allowing you to use async/await with io_uring operations.
///
/// # Example
///
/// ```no_run
/// # async_std::task::block_on(async {
/// use liburing_rs::async_io::AsyncIoUring;
/// use liburing_rs::ops::Nop;
///
/// let mut ring = AsyncIoUring::new(32)?;
///
/// // Submit a NOP operation and await its completion
/// let result = ring.submit_op(Nop).await?;
/// println!("NOP completed with result: {}", result);
/// # Ok::<(), liburing_rs::Error>(())
/// # }).unwrap();
/// ```
pub struct AsyncIoUring {
    ring: Arc<Mutex<IoUring>>,
}

impl AsyncIoUring {
    /// Create a new async io_uring instance with the specified number of entries
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries (will be rounded up to power of 2)
    ///
    /// # Errors
    ///
    /// Returns an error if the kernel doesn't support io_uring or if setup fails.
    pub fn new(entries: u32) -> Result<Self> {
        let ring = IoUring::new(entries)?;
        Ok(Self {
            ring: Arc::new(Mutex::new(ring)),
        })
    }

    /// Submit an operation and wait for its completion asynchronously
    ///
    /// # Arguments
    ///
    /// * `op` - The operation to submit (implements `PrepareOp`)
    ///
    /// # Returns
    ///
    /// A future that resolves to the result code of the operation
    pub async fn submit_op<Op: PrepareOp + Send + 'static>(&mut self, op: Op) -> Result<i32> {
        let ring = self.ring.clone();

        async_std::task::spawn_blocking(move || {
            let mut ring = ring.lock().unwrap();

            // Submit the operation
            let user_data = 1u64;
            {
                let mut sq = ring.submission();
                let sqe = sq.get_sqe_or_err()?;
                op.prepare(sqe);
                sqe.set_user_data(user_data);
            }

            // Submit to kernel
            ring.submit()?;

            // Wait for completion
            let mut cq = ring.completion();
            let cqe = cq.wait_cqe()?;
            Ok(cqe.result())
        })
        .await
    }
}

// AsyncIoUring can be sent between threads
unsafe impl Send for AsyncIoUring {}
unsafe impl Sync for AsyncIoUring {}
