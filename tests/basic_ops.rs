//! Basic operation tests
//! Corresponds to liburing tests: nop.c, fsync.c, close.c

use liburing_rs::{ops::*, IoUring, Result};
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::io::AsRawFd;

#[test]
fn test_nop_single() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Submit a single NOP
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(0x42);
    }

    ring.submit()?;

    // Wait for completion
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;

    assert_eq!(cqe.user_data(), 0x42);
    assert_eq!(cqe.result(), 0);
    assert!(cqe.is_success());

    Ok(())
}

#[test]
fn test_nop_multiple() -> Result<()> {
    let mut ring = IoUring::new(8)?;
    const COUNT: u64 = 5;

    // Submit multiple NOPs
    {
        let mut sq = ring.submission();
        for i in 0..COUNT {
            let sqe = sq.get_sqe_or_err()?;
            Nop.prepare(sqe);
            sqe.set_user_data(i);
        }
    }

    ring.submit()?;

    // Collect all completions
    let mut cq = ring.completion();
    for _ in 0..COUNT {
        let cqe = cq.wait_cqe()?;
        assert!(cqe.is_success());
    }

    Ok(())
}

#[test]
fn test_nop_submit_and_wait() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(123);
    }

    // Submit and wait in one call
    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.peek_cqe().expect("Should have a CQE");
    assert_eq!(cqe.user_data(), 123);

    Ok(())
}

#[test]
fn test_fsync() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut file = OpenOptions::new().write(true).open(tmp.path()).unwrap();

    file.write_all(b"test data").unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Submit fsync
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Fsync::new(fd).prepare(sqe);
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    assert_eq!(cqe.result(), 0);

    Ok(())
}

#[test]
fn test_fsync_datasync() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut file = OpenOptions::new().write(true).open(tmp.path()).unwrap();

    file.write_all(b"test data").unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    // Submit fsync with data sync flag
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Fsync::data_sync(fd).prepare(sqe);
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    assert!(cqe.result() >= 0, "fsync failed: {}", cqe.result());

    Ok(())
}

#[test]
fn test_close() -> Result<()> {
    // Create a file and get its fd
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = File::open(tmp.path()).unwrap();
    let fd = file.as_raw_fd();

    // Need to dup the fd since File will close it on drop
    let dup_fd = unsafe { libc::dup(fd) };
    assert!(dup_fd >= 0);

    let mut ring = IoUring::new(8)?;

    // Submit close operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Close::new(dup_fd).prepare(sqe);
        sqe.set_user_data(10);
    }

    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    assert_eq!(cqe.result(), 0);

    Ok(())
}

#[test]
fn test_peek_cqe() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Submit a NOP
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(99);
    }

    ring.submit()?;

    // Peek should eventually return the CQE
    let mut cq = ring.completion();
    let mut found = false;
    for _ in 0..1000 {
        if let Some(cqe) = cq.peek_cqe() {
            assert_eq!(cqe.user_data(), 99);
            found = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_micros(100));
    }

    assert!(found, "CQE not found after peek attempts");

    Ok(())
}

#[test]
fn test_batch_peek() -> Result<()> {
    let mut ring = IoUring::new(16)?;
    const COUNT: usize = 8;

    // Submit multiple NOPs
    {
        let mut sq = ring.submission();
        for i in 0..COUNT {
            let sqe = sq.get_sqe_or_err()?;
            Nop.prepare(sqe);
            sqe.set_user_data(i as u64);
        }
    }

    ring.submit_and_wait(COUNT as u32)?;

    // Peek batch
    let mut cq = ring.completion();
    let mut cqes = vec![std::ptr::null_mut(); COUNT];
    let n = cq.peek_batch(&mut cqes);

    assert_eq!(n, COUNT, "Expected {} CQEs, got {}", COUNT, n);

    Ok(())
}
