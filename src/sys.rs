//! Low-level FFI bindings to liburing
//!
//! This module contains the raw, unsafe bindings to the liburing C library.
//! Most users should prefer the safe wrappers in the parent module.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(clippy::missing_safety_doc)]

// Include the auto-generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Manual extern declarations for inline functions that exist in liburing-ffi.so
// These are declared as static inline in the headers, so bindgen skips them,
// but they are exported as real functions in liburing-ffi.so
extern "C" {
    // SQE operations
    pub fn io_uring_get_sqe(ring: *mut io_uring) -> *mut io_uring_sqe;
    pub fn io_uring_sqe_set_data(sqe: *mut io_uring_sqe, data: *mut ::std::os::raw::c_void);
    pub fn io_uring_sqe_set_data64(sqe: *mut io_uring_sqe, data: u64);
    pub fn io_uring_sqe_set_flags(sqe: *mut io_uring_sqe, flags: ::std::os::raw::c_uint);
    pub fn io_uring_sqe_set_buf_group(sqe: *mut io_uring_sqe, buf_group: ::std::os::raw::c_ushort);

    // CQE operations
    pub fn io_uring_peek_cqe(
        ring: *mut io_uring,
        cqe_ptr: *mut *mut io_uring_cqe,
    ) -> ::std::os::raw::c_int;
    pub fn io_uring_wait_cqe(
        ring: *mut io_uring,
        cqe_ptr: *mut *mut io_uring_cqe,
    ) -> ::std::os::raw::c_int;
    pub fn io_uring_cqe_seen(ring: *mut io_uring, cqe: *mut io_uring_cqe);
    pub fn io_uring_cq_advance(ring: *mut io_uring, nr: ::std::os::raw::c_uint);
    pub fn io_uring_cqe_get_data(cqe: *const io_uring_cqe) -> *mut ::std::os::raw::c_void;
    pub fn io_uring_cqe_get_data64(cqe: *const io_uring_cqe) -> u64;

    // Prep operations - Basic I/O
    pub fn io_uring_prep_rw(
        op: ::std::os::raw::c_int,
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        addr: *const ::std::os::raw::c_void,
        len: ::std::os::raw::c_uint,
        offset: u64,
    );
    pub fn io_uring_prep_read(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        buf: *mut ::std::os::raw::c_void,
        nbytes: ::std::os::raw::c_uint,
        offset: u64,
    );
    pub fn io_uring_prep_write(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        buf: *const ::std::os::raw::c_void,
        nbytes: ::std::os::raw::c_uint,
        offset: u64,
    );
    pub fn io_uring_prep_readv(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        iovecs: *const libc::iovec,
        nr_vecs: ::std::os::raw::c_uint,
        offset: u64,
    );
    pub fn io_uring_prep_writev(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        iovecs: *const libc::iovec,
        nr_vecs: ::std::os::raw::c_uint,
        offset: u64,
    );
    pub fn io_uring_prep_read_fixed(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        buf: *mut ::std::os::raw::c_void,
        nbytes: ::std::os::raw::c_uint,
        offset: u64,
        buf_index: ::std::os::raw::c_int,
    );
    pub fn io_uring_prep_write_fixed(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        buf: *const ::std::os::raw::c_void,
        nbytes: ::std::os::raw::c_uint,
        offset: u64,
        buf_index: ::std::os::raw::c_int,
    );

    // Prep operations - File operations
    pub fn io_uring_prep_fsync(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        fsync_flags: ::std::os::raw::c_uint,
    );
    pub fn io_uring_prep_close(sqe: *mut io_uring_sqe, fd: ::std::os::raw::c_int);
    pub fn io_uring_prep_openat(
        sqe: *mut io_uring_sqe,
        dfd: ::std::os::raw::c_int,
        path: *const ::std::os::raw::c_char,
        flags: ::std::os::raw::c_int,
        mode: libc::mode_t,
    );

    // Prep operations - Network
    pub fn io_uring_prep_accept(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        addr: *mut libc::sockaddr,
        addrlen: *mut libc::socklen_t,
        flags: ::std::os::raw::c_int,
    );
    pub fn io_uring_prep_connect(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        addr: *const libc::sockaddr,
        addrlen: libc::socklen_t,
    );
    pub fn io_uring_prep_send(
        sqe: *mut io_uring_sqe,
        sockfd: ::std::os::raw::c_int,
        buf: *const ::std::os::raw::c_void,
        len: usize,
        flags: ::std::os::raw::c_int,
    );
    pub fn io_uring_prep_recv(
        sqe: *mut io_uring_sqe,
        sockfd: ::std::os::raw::c_int,
        buf: *mut ::std::os::raw::c_void,
        len: usize,
        flags: ::std::os::raw::c_int,
    );
    pub fn io_uring_prep_sendmsg(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        msg: *const libc::msghdr,
        flags: ::std::os::raw::c_uint,
    );
    pub fn io_uring_prep_recvmsg(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        msg: *mut libc::msghdr,
        flags: ::std::os::raw::c_uint,
    );

    // Prep operations - Other
    pub fn io_uring_prep_nop(sqe: *mut io_uring_sqe);
    pub fn io_uring_prep_timeout(
        sqe: *mut io_uring_sqe,
        ts: *mut __kernel_timespec,
        count: ::std::os::raw::c_uint,
        flags: ::std::os::raw::c_uint,
    );
    pub fn io_uring_prep_poll_add(
        sqe: *mut io_uring_sqe,
        fd: ::std::os::raw::c_int,
        poll_mask: ::std::os::raw::c_uint,
    );
    pub fn io_uring_prep_poll_remove(sqe: *mut io_uring_sqe, user_data: u64);
    pub fn io_uring_prep_cancel(
        sqe: *mut io_uring_sqe,
        user_data: *mut ::std::os::raw::c_void,
        flags: ::std::os::raw::c_int,
    );
    pub fn io_uring_prep_cancel64(
        sqe: *mut io_uring_sqe,
        user_data: u64,
        flags: ::std::os::raw::c_int,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bindings_exist() {
        // Verify that key types are available
        let _sqe_size = std::mem::size_of::<io_uring_sqe>();
        let _cqe_size = std::mem::size_of::<io_uring_cqe>();
        let _ring_size = std::mem::size_of::<io_uring>();
    }

    #[test]
    fn test_sqe_size() {
        // io_uring_sqe should be 64 bytes (or 128 with IORING_SETUP_SQE128)
        assert_eq!(std::mem::size_of::<io_uring_sqe>(), 64);
    }

    #[test]
    fn test_cqe_size() {
        // io_uring_cqe should be 16 bytes (or 32 with IORING_SETUP_CQE32)
        assert_eq!(std::mem::size_of::<io_uring_cqe>(), 16);
    }
}
