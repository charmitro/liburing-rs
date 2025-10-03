//! Submission and completion queue operations

use crate::error::{Error, Result};
use crate::sys;
use std::marker::PhantomData;

/// Submission queue for io_uring
///
/// Used to obtain submission queue entries (SQEs) and submit them to the kernel.
pub struct SubmissionQueue<'ring> {
    ring: *mut sys::io_uring,
    _phantom: PhantomData<&'ring mut sys::io_uring>,
}

impl<'ring> SubmissionQueue<'ring> {
    pub(crate) fn new(ring: &'ring mut sys::io_uring) -> Self {
        Self {
            ring,
            _phantom: PhantomData,
        }
    }

    /// Get the next available submission queue entry
    ///
    /// Returns `None` if the submission queue is full.
    pub fn get_sqe(&mut self) -> Option<&mut sys::io_uring_sqe> {
        let sqe = unsafe { sys::io_uring_get_sqe(self.ring) };

        if sqe.is_null() {
            None
        } else {
            Some(unsafe { &mut *sqe })
        }
    }

    /// Get the next available SQE or return an error if full
    pub fn get_sqe_or_err(&mut self) -> Result<&mut sys::io_uring_sqe> {
        self.get_sqe().ok_or(Error::SubmissionQueueFull)
    }

    /// Submit all pending SQEs to the kernel
    ///
    /// Returns the number of SQEs submitted.
    pub fn submit(&mut self) -> Result<usize> {
        let ret = unsafe { sys::io_uring_submit(self.ring) };

        if ret < 0 {
            Err(crate::error::from_ret_code(ret).into())
        } else {
            Ok(ret as usize)
        }
    }

    /// Submit all pending SQEs and wait for at least `wait_nr` completions
    pub fn submit_and_wait(&mut self, wait_nr: u32) -> Result<usize> {
        let ret = unsafe { sys::io_uring_submit_and_wait(self.ring, wait_nr) };

        if ret < 0 {
            Err(crate::error::from_ret_code(ret).into())
        } else {
            Ok(ret as usize)
        }
    }

    /// Get the number of SQE slots available
    pub fn space_left(&self) -> u32 {
        unsafe {
            let sq = &(*self.ring).sq;
            let head = *sq.khead;
            let tail = sq.sqe_tail;
            sq.ring_entries - (tail.wrapping_sub(head))
        }
    }

    /// Check if the submission queue is full
    pub fn is_full(&self) -> bool {
        self.space_left() == 0
    }
}

/// Completion queue for io_uring
///
/// Used to retrieve and process completion queue entries (CQEs).
pub struct CompletionQueue<'ring> {
    ring: *mut sys::io_uring,
    _phantom: PhantomData<&'ring mut sys::io_uring>,
}

impl<'ring> CompletionQueue<'ring> {
    pub(crate) fn new(ring: &'ring mut sys::io_uring) -> Self {
        Self {
            ring,
            _phantom: PhantomData,
        }
    }

    /// Wait for a completion queue entry
    ///
    /// This blocks until at least one CQE is available.
    pub fn wait_cqe(&mut self) -> Result<Cqe<'_>> {
        let mut cqe: *mut sys::io_uring_cqe = std::ptr::null_mut();

        let ret =
            unsafe { sys::io_uring_wait_cqe_timeout(self.ring, &mut cqe, std::ptr::null_mut()) };

        if ret < 0 {
            Err(crate::error::from_ret_code(ret).into())
        } else if cqe.is_null() {
            Err(Error::CompletionQueueEmpty)
        } else {
            Ok(Cqe {
                cqe,
                ring: self.ring,
                _phantom: PhantomData,
            })
        }
    }

    /// Peek at a completion queue entry without blocking
    ///
    /// Returns `None` if no CQEs are available.
    pub fn peek_cqe(&mut self) -> Option<Cqe<'_>> {
        let mut cqe: *mut sys::io_uring_cqe = std::ptr::null_mut();

        let ret = unsafe { sys::io_uring_peek_cqe(self.ring, &mut cqe) };

        if ret < 0 || cqe.is_null() {
            None
        } else {
            Some(Cqe {
                cqe,
                ring: self.ring,
                _phantom: PhantomData,
            })
        }
    }

    /// Peek at multiple CQEs at once
    ///
    /// Returns the number of CQEs peeked (up to `count`).
    pub fn peek_batch(&mut self, cqes: &mut [*mut sys::io_uring_cqe]) -> usize {
        let count = cqes.len() as u32;

        unsafe { sys::io_uring_peek_batch_cqe(self.ring, cqes.as_mut_ptr(), count) as usize }
    }
}

/// A single completion queue entry
///
/// When dropped, the CQE is automatically marked as seen.
pub struct Cqe<'ring> {
    cqe: *mut sys::io_uring_cqe,
    ring: *mut sys::io_uring,
    _phantom: PhantomData<&'ring mut sys::io_uring_cqe>,
}

impl<'ring> Cqe<'ring> {
    /// Get the user data that was set on the SQE
    pub fn user_data(&self) -> u64 {
        unsafe { (*self.cqe).user_data }
    }

    /// Get the result code for this operation
    ///
    /// Negative values indicate errors (errno values).
    pub fn result(&self) -> i32 {
        unsafe { (*self.cqe).res }
    }

    /// Get the flags for this CQE
    pub fn flags(&self) -> u32 {
        unsafe { (*self.cqe).flags }
    }

    /// Check if the operation was successful
    pub fn is_success(&self) -> bool {
        self.result() >= 0
    }

    /// Convert the result to a `std::io::Result`
    pub fn into_result(self) -> std::io::Result<i32> {
        let res = self.result();
        if res < 0 {
            Err(crate::error::from_ret_code(res))
        } else {
            Ok(res)
        }
    }
}

impl Drop for Cqe<'_> {
    fn drop(&mut self) {
        // Mark the CQE as seen
        unsafe {
            sys::io_uring_cqe_seen(self.ring, self.cqe);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_queue_operations() {
        // Basic smoke test - actual operations require a working io_uring
        // which may not be available in all test environments
    }
}
