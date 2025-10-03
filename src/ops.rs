//! Operation preparation helpers
//!
//! This module provides safe wrappers around io_uring operation preparation functions.

use crate::sys;
use std::os::unix::io::RawFd;

/// Helper trait for preparing operations on SQEs
pub trait PrepareOp {
    /// Prepare this operation on the given SQE
    fn prepare(&self, sqe: &mut sys::io_uring_sqe);
}

/// Read operation
pub struct Read {
    /// File descriptor to read from
    pub fd: RawFd,
    /// Buffer to read into
    pub buf: *mut u8,
    /// Number of bytes to read
    pub len: u32,
    /// Offset in the file to read from
    pub offset: u64,
}

impl Read {
    /// Create a new read operation
    ///
    /// # Safety
    ///
    /// The buffer must be valid and live until the operation completes.
    pub unsafe fn new(fd: RawFd, buf: *mut u8, len: u32, offset: u64) -> Self {
        Self {
            fd,
            buf,
            len,
            offset,
        }
    }

    /// Create a read operation from a byte slice
    pub fn from_slice(fd: RawFd, buf: &mut [u8], offset: u64) -> Self {
        Self {
            fd,
            buf: buf.as_mut_ptr(),
            len: buf.len() as u32,
            offset,
        }
    }
}

impl PrepareOp for Read {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_read(
                sqe,
                self.fd,
                self.buf as *mut std::ffi::c_void,
                self.len,
                self.offset,
            );
        }
    }
}

/// Write operation
pub struct Write {
    /// File descriptor to write to
    pub fd: RawFd,
    /// Buffer to write from
    pub buf: *const u8,
    /// Number of bytes to write
    pub len: u32,
    /// Offset in the file to write to
    pub offset: u64,
}

impl Write {
    /// Create a new write operation
    ///
    /// # Safety
    ///
    /// The buffer must be valid and live until the operation completes.
    pub unsafe fn new(fd: RawFd, buf: *const u8, len: u32, offset: u64) -> Self {
        Self {
            fd,
            buf,
            len,
            offset,
        }
    }

    /// Create a write operation from a byte slice
    pub fn from_slice(fd: RawFd, buf: &[u8], offset: u64) -> Self {
        Self {
            fd,
            buf: buf.as_ptr(),
            len: buf.len() as u32,
            offset,
        }
    }
}

impl PrepareOp for Write {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_write(
                sqe,
                self.fd,
                self.buf as *const std::ffi::c_void,
                self.len,
                self.offset,
            );
        }
    }
}

/// Fsync operation
pub struct Fsync {
    /// File descriptor to sync
    pub fd: RawFd,
    /// Fsync flags
    pub flags: u32,
}

impl Fsync {
    /// Create a new fsync operation
    pub fn new(fd: RawFd) -> Self {
        Self { fd, flags: 0 }
    }

    /// Create an fsync operation with data-only sync
    pub fn data_sync(fd: RawFd) -> Self {
        Self {
            fd,
            flags: sys::IORING_FSYNC_DATASYNC,
        }
    }
}

impl PrepareOp for Fsync {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_fsync(sqe, self.fd, self.flags);
        }
    }
}

/// NOP operation (for testing)
pub struct Nop;

impl PrepareOp for Nop {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_nop(sqe);
        }
    }
}

/// Accept operation
pub struct Accept {
    /// Socket file descriptor
    pub fd: RawFd,
    /// Address buffer
    pub addr: *mut libc::sockaddr,
    /// Address length
    pub addrlen: *mut libc::socklen_t,
    /// Accept flags
    pub flags: i32,
}

impl Accept {
    /// Create a new accept operation
    ///
    /// # Safety
    ///
    /// The addr and addrlen pointers must be valid until the operation completes.
    pub unsafe fn new(
        fd: RawFd,
        addr: *mut libc::sockaddr,
        addrlen: *mut libc::socklen_t,
        flags: i32,
    ) -> Self {
        Self {
            fd,
            addr,
            addrlen,
            flags,
        }
    }
}

impl PrepareOp for Accept {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_accept(sqe, self.fd, self.addr, self.addrlen, self.flags);
        }
    }
}

/// Connect operation
pub struct Connect {
    /// Socket file descriptor
    pub fd: RawFd,
    /// Address to connect to
    pub addr: *const libc::sockaddr,
    /// Address length
    pub addrlen: libc::socklen_t,
}

impl Connect {
    /// Create a new connect operation
    ///
    /// # Safety
    ///
    /// The addr pointer must be valid until the operation completes.
    pub unsafe fn new(fd: RawFd, addr: *const libc::sockaddr, addrlen: libc::socklen_t) -> Self {
        Self { fd, addr, addrlen }
    }
}

impl PrepareOp for Connect {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_connect(sqe, self.fd, self.addr, self.addrlen);
        }
    }
}

/// Close operation
pub struct Close {
    /// File descriptor to close
    pub fd: RawFd,
}

impl Close {
    /// Create a new close operation
    pub fn new(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl PrepareOp for Close {
    fn prepare(&self, sqe: &mut sys::io_uring_sqe) {
        unsafe {
            sys::io_uring_prep_close(sqe, self.fd);
        }
    }
}

/// Extension methods for io_uring_sqe
pub trait SqeExt {
    /// Set user data on this SQE
    fn set_user_data(&mut self, data: u64);

    /// Set flags on this SQE
    fn set_flags(&mut self, flags: u8);
}

impl SqeExt for sys::io_uring_sqe {
    fn set_user_data(&mut self, data: u64) {
        unsafe {
            sys::io_uring_sqe_set_data64(self, data);
        }
    }

    fn set_flags(&mut self, flags: u8) {
        unsafe {
            sys::io_uring_sqe_set_flags(self, flags as u32);
        }
    }
}
