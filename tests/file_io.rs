//! File I/O operation tests
//! Corresponds to liburing tests: read-write.c, readv.c, writev.c, read-write-fixed.c

use liburing_rs::{ops::*, IoUring, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read as IoRead, Write as IoWrite};
use std::os::unix::io::AsRawFd;

const TEST_DATA: &[u8] = b"Hello, io_uring world! This is test data for read/write operations.";

#[test]
fn test_read_single_buffer() -> Result<()> {
    // Create test file
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(TEST_DATA).unwrap();
    tmp.flush().unwrap();

    // Open for reading
    let file = File::open(tmp.path()).unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;
    let mut buffer = vec![0u8; TEST_DATA.len()];

    // Submit read operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Read::from_slice(fd, &mut buffer, 0).prepare(sqe);
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;

    // Get result
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    let bytes_read = cqe.result();

    assert!(bytes_read > 0, "Read failed: {}", bytes_read);
    assert_eq!(bytes_read as usize, TEST_DATA.len());
    assert_eq!(&buffer[..], TEST_DATA);

    Ok(())
}

#[test]
fn test_write_single_buffer() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    // Open for writing
    let file = OpenOptions::new().write(true).open(tmp.path()).unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Submit write operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Write::from_slice(fd, TEST_DATA, 0).prepare(sqe);
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;

    // Get result
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    let bytes_written = cqe.result();

    assert_eq!(bytes_written as usize, TEST_DATA.len());

    // Verify data was written
    drop(file);
    let mut verify_file = File::open(tmp.path()).unwrap();
    let mut verify_buf = Vec::new();
    verify_file.read_to_end(&mut verify_buf).unwrap();
    assert_eq!(&verify_buf[..], TEST_DATA);

    Ok(())
}

#[test]
fn test_read_write_at_offset() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path())
        .unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Write at offset 100
    const OFFSET: u64 = 100;
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Write::from_slice(fd, TEST_DATA, OFFSET).prepare(sqe);
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, TEST_DATA.len());
    }

    // Read back from same offset
    let mut buffer = vec![0u8; TEST_DATA.len()];
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Read::from_slice(fd, &mut buffer, OFFSET).prepare(sqe);
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, TEST_DATA.len());
    }

    assert_eq!(&buffer[..], TEST_DATA);

    Ok(())
}

#[test]
fn test_readv_writev() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path())
        .unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Prepare scatter-gather buffers
    let data1 = b"First part ";
    let data2 = b"Second part ";
    let data3 = b"Third part";

    let iovecs = [
        libc::iovec {
            iov_base: data1.as_ptr() as *mut _,
            iov_len: data1.len(),
        },
        libc::iovec {
            iov_base: data2.as_ptr() as *mut _,
            iov_len: data2.len(),
        },
        libc::iovec {
            iov_base: data3.as_ptr() as *mut _,
            iov_len: data3.len(),
        },
    ];

    // Submit writev
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_writev(
                sqe,
                fd,
                iovecs.as_ptr(),
                iovecs.len() as u32,
                0,
            );
        }
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;
    let total_len = data1.len() + data2.len() + data3.len();
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, total_len);
    }

    // Read back with readv
    let mut buf1 = vec![0u8; data1.len()];
    let mut buf2 = vec![0u8; data2.len()];
    let mut buf3 = vec![0u8; data3.len()];

    let read_iovecs = [
        libc::iovec {
            iov_base: buf1.as_mut_ptr() as *mut _,
            iov_len: buf1.len(),
        },
        libc::iovec {
            iov_base: buf2.as_mut_ptr() as *mut _,
            iov_len: buf2.len(),
        },
        libc::iovec {
            iov_base: buf3.as_mut_ptr() as *mut _,
            iov_len: buf3.len(),
        },
    ];

    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_readv(
                sqe,
                fd,
                read_iovecs.as_ptr(),
                read_iovecs.len() as u32,
                0,
            );
        }
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, total_len);
    }

    assert_eq!(&buf1[..], data1);
    assert_eq!(&buf2[..], data2);
    assert_eq!(&buf3[..], data3);

    Ok(())
}

#[test]
fn test_multiple_reads() -> Result<()> {
    // Create test file with known pattern
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let test_pattern = b"0123456789ABCDEF";
    for _ in 0..10 {
        tmp.write_all(test_pattern).unwrap();
    }
    tmp.flush().unwrap();

    let file = File::open(tmp.path()).unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(16)?;

    // Submit multiple read operations at different offsets
    const NUM_READS: usize = 5;
    let mut buffers: Vec<Vec<u8>> = (0..NUM_READS)
        .map(|_| vec![0u8; test_pattern.len()])
        .collect();

    {
        let mut sq = ring.submission();
        for (i, buf) in buffers.iter_mut().enumerate() {
            let sqe = sq.get_sqe_or_err()?;
            let offset = (i * test_pattern.len()) as u64;
            Read::from_slice(fd, buf, offset).prepare(sqe);
            sqe.set_user_data(i as u64);
        }
    }

    ring.submit_and_wait(NUM_READS as u32)?;

    // Collect completions
    let mut cq = ring.completion();
    for _ in 0..NUM_READS {
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, test_pattern.len());
    }

    // Verify all buffers have the pattern
    for buf in &buffers {
        assert_eq!(&buf[..], test_pattern);
    }

    Ok(())
}

#[test]
fn test_large_io() -> Result<()> {
    const SIZE: usize = 1024 * 1024; // 1MB
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path())
        .unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Create 1MB of test data
    let test_data: Vec<u8> = (0..SIZE).map(|i| (i % 256) as u8).collect();

    // Write 1MB
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Write::from_slice(fd, &test_data, 0).prepare(sqe);
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, SIZE);
    }

    // Read 1MB back
    let mut read_buf = vec![0u8; SIZE];
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Read::from_slice(fd, &mut read_buf, 0).prepare(sqe);
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, SIZE);
    }

    assert_eq!(read_buf, test_data);

    Ok(())
}

#[test]
fn test_sequential_operations() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path())
        .unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Write some data
    let data = b"Sequential test data";
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Write::from_slice(fd, data, 0).prepare(sqe);
        sqe.set_user_data(1);
    }
    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, data.len());
    }

    // Fsync
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Fsync::new(fd).prepare(sqe);
        sqe.set_user_data(2);
    }
    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result(), 0);
    }

    // Read it back
    let mut buffer = vec![0u8; data.len()];
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Read::from_slice(fd, &mut buffer, 0).prepare(sqe);
        sqe.set_user_data(3);
    }
    ring.submit_and_wait(1)?;
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, data.len());
    }

    assert_eq!(&buffer[..], data);

    Ok(())
}
