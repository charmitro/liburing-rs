//! Tokio async runtime integration for io_uring

use crate::{
    ops::{PrepareOp, SqeExt},
    Error, IoUring, Result,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

/// Async io_uring instance integrated with tokio runtime
///
/// This wraps an `IoUring` instance and integrates it with tokio's async runtime,
/// allowing you to use async/await with io_uring operations.
///
/// # Example
///
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use liburing_rs::async_io::AsyncIoUring;
/// use liburing_rs::ops::Nop;
///
/// let mut ring = AsyncIoUring::new(32)?;
///
/// // Submit a NOP operation and await its completion
/// let result = ring.submit_op(Nop).await?;
/// println!("NOP completed with result: {}", result);
/// # Ok(())
/// # }
/// ```
pub struct AsyncIoUring {
    inner: Arc<Mutex<AsyncIoUringInner>>,
}

struct AsyncIoUringInner {
    ring: IoUring,
    async_fd: AsyncFd<RawFdWrapper>,
    wakers: HashMap<u64, Waker>,
    next_user_data: u64,
}

/// Wrapper to make RawFd work with AsyncFd
struct RawFdWrapper(std::os::unix::io::RawFd);

impl std::os::unix::io::AsRawFd for RawFdWrapper {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.0
    }
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
        let fd = ring.as_raw_fd();

        // Wrap the fd in our wrapper type
        let fd_wrapper = RawFdWrapper(fd);

        // Create AsyncFd with READABLE interest (io_uring fd becomes readable when completions arrive)
        let async_fd = AsyncFd::with_interest(fd_wrapper, Interest::READABLE).map_err(Error::Io)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(AsyncIoUringInner {
                ring,
                async_fd,
                wakers: HashMap::new(),
                next_user_data: 1,
            })),
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
    pub fn submit_op<Op: PrepareOp + 'static>(
        &mut self,
        op: Op,
    ) -> impl Future<Output = Result<i32>> {
        SubmitFuture {
            ring: self.inner.clone(),
            op: Some(op),
            user_data: None,
        }
    }
}

struct SubmitFuture<Op> {
    ring: Arc<Mutex<AsyncIoUringInner>>,
    op: Option<Op>,
    user_data: Option<u64>,
}

// SubmitFuture doesn't need to be pinned
impl<Op> Unpin for SubmitFuture<Op> {}

impl<Op: PrepareOp> Future for SubmitFuture<Op> {
    type Output = Result<i32>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let future = self.get_mut();
        let mut inner = future.ring.lock().unwrap();

        // If we haven't submitted yet, do so now
        if let Some(op) = future.op.take() {
            // Get the next user_data value
            let user_data = inner.next_user_data;
            inner.next_user_data = inner.next_user_data.wrapping_add(1);
            future.user_data = Some(user_data);

            // Submit the operation
            {
                let mut sq = inner.ring.submission();
                let sqe = match sq.get_sqe_or_err() {
                    Ok(sqe) => sqe,
                    Err(e) => return Poll::Ready(Err(e)),
                };
                op.prepare(sqe);
                sqe.set_user_data(user_data);
            }

            // Submit to kernel
            if let Err(e) = inner.ring.submit() {
                return Poll::Ready(Err(e));
            }

            // Register our waker
            inner.wakers.insert(user_data, cx.waker().clone());
        }

        // Try to get completion
        let user_data = future.user_data.unwrap();

        // Check for completions
        loop {
            let cqe_data: Option<(u64, i32)> = {
                let mut cq = inner.ring.completion();
                cq.peek_cqe().map(|cqe| (cqe.user_data(), cqe.result()))
            };

            match cqe_data {
                Some((cqe_user_data, result)) => {
                    if cqe_user_data == user_data {
                        // This is our completion
                        inner.wakers.remove(&user_data);
                        return Poll::Ready(Ok(result));
                    } else {
                        // Wake up the other task
                        if let Some(waker) = inner.wakers.remove(&cqe_user_data) {
                            waker.wake();
                        }
                    }
                }
                None => {
                    // No more completions available, wait for fd to become readable
                    break;
                }
            }
        }

        // Wait for the fd to become readable (more completions available)
        match inner.async_fd.poll_read_ready(cx) {
            Poll::Ready(Ok(mut guard)) => {
                // Clear the ready state
                guard.clear_ready();
                // Re-register our waker and return pending
                // The next poll will check for completions again
                inner.wakers.insert(user_data, cx.waker().clone());
                Poll::Pending
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::Io(e))),
            Poll::Pending => {
                // Make sure our waker is registered
                inner.wakers.insert(user_data, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

// AsyncIoUring can be sent between threads
unsafe impl Send for AsyncIoUring {}
unsafe impl Sync for AsyncIoUring {}
