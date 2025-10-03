//! Async operation tests

#[cfg(feature = "async-tokio")]
mod tokio_tests {
    use liburing_rs::async_io::tokio_impl::AsyncIoUring;
    use liburing_rs::ops::{Nop, PrepareOp};
    use liburing_rs::Result;
    use std::os::unix::io::AsRawFd;

    #[tokio::test]
    async fn test_async_nop() -> Result<()> {
        let mut ring = AsyncIoUring::new(8)?;
        let result = ring.submit_op(Nop).await?;
        assert_eq!(result, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_async_multiple_nops() -> Result<()> {
        let mut ring = AsyncIoUring::new(8)?;

        for _ in 0..5 {
            let result = ring.submit_op(Nop).await?;
            assert_eq!(result, 0);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_async_send_recv() -> Result<()> {
        // Create a socket pair
        let mut fds = [0i32; 2];
        let ret =
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
        assert_eq!(ret, 0);

        let (sock1, sock2) = (fds[0], fds[1]);

        let mut ring = AsyncIoUring::new(8)?;

        // Send data
        let send_data = b"Hello async io_uring!";

        // Prepare send operation
        struct SendOp {
            fd: i32,
            data: &'static [u8],
        }

        impl PrepareOp for SendOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_send(
                        sqe,
                        self.fd,
                        self.data.as_ptr() as *const _,
                        self.data.len(),
                        0,
                    );
                }
            }
        }

        let send_result = ring
            .submit_op(SendOp {
                fd: sock1,
                data: send_data,
            })
            .await?;
        assert_eq!(send_result as usize, send_data.len());

        // Receive data
        let mut recv_buf = vec![0u8; send_data.len()];
        let buf_ptr = recv_buf.as_mut_ptr();
        let buf_len = recv_buf.len();

        struct RecvOp {
            fd: i32,
            buf: *mut u8,
            len: usize,
        }

        unsafe impl Send for RecvOp {}

        impl PrepareOp for RecvOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_recv(
                        sqe,
                        self.fd,
                        self.buf as *mut _,
                        self.len,
                        0,
                    );
                }
            }
        }

        let recv_result = ring
            .submit_op(RecvOp {
                fd: sock2,
                buf: buf_ptr,
                len: buf_len,
            })
            .await?;
        assert_eq!(recv_result as usize, send_data.len());
        assert_eq!(&recv_buf[..], send_data);

        // Clean up
        unsafe {
            libc::close(sock1);
            libc::close(sock2);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_async_file_io() -> Result<()> {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("async_test.txt");

        // Write some data synchronously
        let test_data = b"Async file I/O test";
        {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(test_data).unwrap();
        }

        // Open for reading
        let file = File::open(&file_path).unwrap();
        let fd = file.as_raw_fd();

        let mut ring = AsyncIoUring::new(8)?;

        // Read data asynchronously
        let mut read_buf = vec![0u8; test_data.len()];
        let buf_ptr = read_buf.as_mut_ptr();
        let buf_len = read_buf.len();

        struct ReadOp {
            fd: i32,
            buf: *mut u8,
            len: usize,
            offset: u64,
        }

        unsafe impl Send for ReadOp {}

        impl PrepareOp for ReadOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_read(
                        sqe,
                        self.fd,
                        self.buf as *mut _,
                        self.len as u32,
                        self.offset,
                    );
                }
            }
        }

        let result = ring
            .submit_op(ReadOp {
                fd,
                buf: buf_ptr,
                len: buf_len,
                offset: 0,
            })
            .await?;

        assert_eq!(result as usize, test_data.len());
        assert_eq!(&read_buf[..], test_data);

        Ok(())
    }
}

#[cfg(feature = "async-async-std")]
mod async_std_tests {
    use liburing_rs::async_io::async_std_impl::AsyncIoUring;
    use liburing_rs::ops::{Nop, PrepareOp};
    use liburing_rs::Result;
    use std::os::unix::io::AsRawFd;

    #[async_std::test]
    async fn test_async_nop() -> Result<()> {
        let mut ring = AsyncIoUring::new(8)?;
        let result = ring.submit_op(Nop).await?;
        assert_eq!(result, 0);
        Ok(())
    }

    #[async_std::test]
    async fn test_async_multiple_nops() -> Result<()> {
        let mut ring = AsyncIoUring::new(8)?;

        for _ in 0..5 {
            let result = ring.submit_op(Nop).await?;
            assert_eq!(result, 0);
        }

        Ok(())
    }

    #[async_std::test]
    async fn test_async_send_recv() -> Result<()> {
        // Create a socket pair
        let mut fds = [0i32; 2];
        let ret =
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
        assert_eq!(ret, 0);

        let (sock1, sock2) = (fds[0], fds[1]);

        let mut ring = AsyncIoUring::new(8)?;

        // Send data
        let send_data = b"Hello async io_uring!";

        // Prepare send operation
        struct SendOp {
            fd: i32,
            data: &'static [u8],
        }

        impl PrepareOp for SendOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_send(
                        sqe,
                        self.fd,
                        self.data.as_ptr() as *const _,
                        self.data.len(),
                        0,
                    );
                }
            }
        }

        let send_result = ring
            .submit_op(SendOp {
                fd: sock1,
                data: send_data,
            })
            .await?;
        assert_eq!(send_result as usize, send_data.len());

        // Receive data
        let mut recv_buf = vec![0u8; send_data.len()];
        let buf_ptr = recv_buf.as_mut_ptr();
        let buf_len = recv_buf.len();

        struct RecvOp {
            fd: i32,
            buf: *mut u8,
            len: usize,
        }

        unsafe impl Send for RecvOp {}

        impl PrepareOp for RecvOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_recv(
                        sqe,
                        self.fd,
                        self.buf as *mut _,
                        self.len,
                        0,
                    );
                }
            }
        }

        let recv_result = ring
            .submit_op(RecvOp {
                fd: sock2,
                buf: buf_ptr,
                len: buf_len,
            })
            .await?;
        assert_eq!(recv_result as usize, send_data.len());
        assert_eq!(&recv_buf[..], send_data);

        // Clean up
        unsafe {
            libc::close(sock1);
            libc::close(sock2);
        }

        Ok(())
    }

    #[async_std::test]
    async fn test_async_file_io() -> Result<()> {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("async_test.txt");

        // Write some data synchronously
        let test_data = b"Async file I/O test";
        {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(test_data).unwrap();
        }

        // Open for reading
        let file = File::open(&file_path).unwrap();
        let fd = file.as_raw_fd();

        let mut ring = AsyncIoUring::new(8)?;

        // Read data asynchronously
        let mut read_buf = vec![0u8; test_data.len()];
        let buf_ptr = read_buf.as_mut_ptr();
        let buf_len = read_buf.len();

        struct ReadOp {
            fd: i32,
            buf: *mut u8,
            len: usize,
            offset: u64,
        }

        unsafe impl Send for ReadOp {}

        impl PrepareOp for ReadOp {
            fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
                unsafe {
                    liburing_rs::sys::io_uring_prep_read(
                        sqe,
                        self.fd,
                        self.buf as *mut _,
                        self.len as u32,
                        self.offset,
                    );
                }
            }
        }

        let result = ring
            .submit_op(ReadOp {
                fd,
                buf: buf_ptr,
                len: buf_len,
                offset: 0,
            })
            .await?;

        assert_eq!(result as usize, test_data.len());
        assert_eq!(&read_buf[..], test_data);

        Ok(())
    }
}
