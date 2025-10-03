//! Main IoUring struct and setup operations

use crate::error::{check_ret, Error, Result};
use crate::flags::SetupFlags;
use crate::queue::{CompletionQueue, SubmissionQueue};
use crate::sys;
use std::mem::MaybeUninit;
use std::os::unix::io::{AsRawFd, RawFd};

/// The main io_uring instance
///
/// This provides a safe wrapper around the `io_uring` C struct.
/// The ring is automatically cleaned up when dropped.
///
/// # Example
///
/// ```no_run
/// use liburing_rs::IoUring;
///
/// let ring = IoUring::new(32)?;
/// # Ok::<(), liburing_rs::Error>(())
/// ```
pub struct IoUring {
    ring: sys::io_uring,
}

impl IoUring {
    /// Create a new io_uring instance with the specified number of entries
    ///
    /// The number of entries will be rounded up to the nearest power of 2.
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries (will be rounded up to power of 2)
    ///
    /// # Errors
    ///
    /// Returns an error if the kernel doesn't support io_uring or if setup fails.
    pub fn new(entries: u32) -> Result<Self> {
        Self::with_flags(entries, SetupFlags::empty())
    }

    /// Create a new io_uring instance with specific setup flags
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries
    /// * `flags` - Setup flags to configure the ring
    ///
    /// # Example
    ///
    /// ```no_run
    /// use liburing_rs::{IoUring, flags::SetupFlags};
    ///
    /// let ring = IoUring::with_flags(32, SetupFlags::SQPOLL)?;
    /// # Ok::<(), liburing_rs::Error>(())
    /// ```
    pub fn with_flags(entries: u32, flags: SetupFlags) -> Result<Self> {
        let mut ring = MaybeUninit::<sys::io_uring>::uninit();

        let ret = unsafe { sys::io_uring_queue_init(entries, ring.as_mut_ptr(), flags.bits()) };

        check_ret(ret).map_err(Error::Setup)?;

        Ok(Self {
            ring: unsafe { ring.assume_init() },
        })
    }

    /// Create a new io_uring instance with custom parameters
    ///
    /// This provides the most control over ring configuration.
    ///
    /// # Arguments
    ///
    /// * `entries` - Number of submission queue entries
    /// * `params` - Custom io_uring parameters
    pub fn with_params(entries: u32, params: &mut sys::io_uring_params) -> Result<Self> {
        let mut ring = MaybeUninit::<sys::io_uring>::uninit();

        let ret = unsafe { sys::io_uring_queue_init_params(entries, ring.as_mut_ptr(), params) };

        check_ret(ret).map_err(Error::Setup)?;

        Ok(Self {
            ring: unsafe { ring.assume_init() },
        })
    }

    /// Submit all queued submission queue entries
    ///
    /// Returns the number of submitted entries.
    pub fn submit(&mut self) -> Result<usize> {
        let ret = unsafe { sys::io_uring_submit(&mut self.ring) };
        check_ret(ret).map(|n| n as usize).map_err(Into::into)
    }

    /// Submit entries and wait for at least `wait_nr` completions
    ///
    /// # Arguments
    ///
    /// * `wait_nr` - Minimum number of completions to wait for
    pub fn submit_and_wait(&mut self, wait_nr: u32) -> Result<usize> {
        let ret = unsafe { sys::io_uring_submit_and_wait(&mut self.ring, wait_nr) };
        check_ret(ret).map(|n| n as usize).map_err(Into::into)
    }

    /// Get a reference to the submission queue
    pub fn submission(&mut self) -> SubmissionQueue<'_> {
        SubmissionQueue::new(&mut self.ring)
    }

    /// Get a reference to the completion queue
    pub fn completion(&mut self) -> CompletionQueue<'_> {
        CompletionQueue::new(&mut self.ring)
    }

    /// Get the raw io_uring pointer (for advanced usage)
    ///
    /// # Safety
    ///
    /// The caller must ensure they don't invalidate the ring's state.
    pub unsafe fn as_raw_mut(&mut self) -> *mut sys::io_uring {
        &mut self.ring
    }

    /// Get the ring file descriptor
    pub fn as_raw_fd(&self) -> RawFd {
        self.ring.ring_fd
    }
}

impl AsRawFd for IoUring {
    fn as_raw_fd(&self) -> RawFd {
        self.ring.ring_fd
    }
}

impl Drop for IoUring {
    fn drop(&mut self) {
        unsafe {
            sys::io_uring_queue_exit(&mut self.ring);
        }
    }
}

// io_uring is safe to send between threads once created
unsafe impl Send for IoUring {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ring() {
        let ring = IoUring::new(8);
        assert!(ring.is_ok(), "Failed to create io_uring");
    }

    #[test]
    fn test_create_with_flags() {
        let ring = IoUring::with_flags(8, SetupFlags::CLAMP);
        assert!(ring.is_ok(), "Failed to create io_uring with flags");
    }
}
