//! Advanced io_uring feature tests
//! Corresponds to liburing tests: poll.c, timeout.c, link.c, cancel.c

use liburing_rs::{flags::SqeFlags, ops::*, IoUring, Result};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

#[test]
fn test_timeout() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Prepare timeout spec (100ms)
    let mut ts = liburing_rs::sys::__kernel_timespec {
        tv_sec: 0,
        tv_nsec: 100_000_000, // 100ms
    };

    // Submit timeout operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_timeout(sqe, &mut ts as *mut _, 0, 0);
        }
        sqe.set_user_data(1);
    }

    let start = std::time::Instant::now();
    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;

    let elapsed = start.elapsed();

    // Result should be -ETIME (timeout expired)
    assert_eq!(cqe.result(), -libc::ETIME);

    // Should have waited ~100ms
    assert!(elapsed >= Duration::from_millis(90));
    assert!(elapsed < Duration::from_millis(200));

    Ok(())
}

#[test]
fn test_timeout_count() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Timeout after 2 completions
    let mut ts = liburing_rs::sys::__kernel_timespec {
        tv_sec: 10,
        tv_nsec: 0,
    };

    {
        let mut sq = ring.submission();

        // Submit timeout waiting for 2 completions
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_timeout(sqe, &mut ts as *mut _, 2, 0);
        }
        sqe.set_user_data(100);

        // Submit 2 NOPs
        for i in 0..2 {
            let sqe = sq.get_sqe_or_err()?;
            Nop.prepare(sqe);
            sqe.set_user_data(i);
        }
    }

    ring.submit()?;

    // Get all completions
    let mut cq = ring.completion();
    let mut count = 0;
    let mut timeout_seen = false;

    for _ in 0..3 {
        let cqe = cq.wait_cqe()?;
        if cqe.user_data() == 100 {
            timeout_seen = true;
            // Should complete successfully after 2 ops
            assert_eq!(cqe.result(), 0);
        }
        count += 1;
    }

    assert_eq!(count, 3);
    assert!(timeout_seen);

    Ok(())
}

#[test]
fn test_poll_fd() -> Result<()> {
    // Create a pipe
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(ret, 0);

    let (read_fd, write_fd) = (fds[0], fds[1]);

    let mut ring = IoUring::new(8)?;

    // Submit poll for POLLIN on read end
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_poll_add(sqe, read_fd, libc::POLLIN as u32);
        }
        sqe.set_user_data(1);
    }

    ring.submit()?;

    // Write to pipe from another thread
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        unsafe {
            libc::write(write_fd, b"x".as_ptr() as *const _, 1);
            libc::close(write_fd);
        }
    });

    // Wait for poll to complete
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;

    // Should have POLLIN set
    let revents = cqe.result() as u32;
    assert!(revents & (libc::POLLIN as u32) != 0);

    unsafe {
        libc::close(read_fd);
    }

    Ok(())
}

#[test]
fn test_linked_operations() -> Result<()> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path())
        .unwrap();
    let fd = file.as_raw_fd();

    let mut ring = IoUring::new(8)?;

    let data = b"Linked test";

    // Submit write linked with fsync
    {
        let mut sq = ring.submission();

        // Write operation with LINK flag
        let sqe = sq.get_sqe_or_err()?;
        Write::from_slice(fd, data, 0).prepare(sqe);
        sqe.set_flags(SqeFlags::IO_LINK.bits());
        sqe.set_user_data(1);

        // Fsync operation (will run after write completes)
        let sqe = sq.get_sqe_or_err()?;
        Fsync::new(fd).prepare(sqe);
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(2)?;

    // Both should complete
    let mut cq = ring.completion();
    let cqe1 = cq.wait_cqe()?;
    assert_eq!(cqe1.user_data(), 1);
    assert_eq!(cqe1.result() as usize, data.len());
    drop(cqe1);

    let cqe2 = cq.wait_cqe()?;
    assert_eq!(cqe2.user_data(), 2);
    assert_eq!(cqe2.result(), 0);

    Ok(())
}

#[test]
fn test_cancel_operation() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Submit a timeout that we'll cancel
    let mut ts = liburing_rs::sys::__kernel_timespec {
        tv_sec: 100,
        tv_nsec: 0,
    };

    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_timeout(sqe, &mut ts as *mut _, 0, 0);
        }
        sqe.set_user_data(0x1234);
    }

    ring.submit()?;

    // Submit cancel
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_cancel64(sqe, 0x1234, 0);
        }
        sqe.set_user_data(0x5678);
    }

    ring.submit_and_wait(2)?;

    // Get completions
    let mut cq = ring.completion();

    let mut cancel_ok = false;
    let mut timeout_canceled = false;

    for _ in 0..2 {
        let cqe = cq.wait_cqe()?;
        if cqe.user_data() == 0x5678 {
            // Cancel operation should succeed
            cancel_ok = cqe.result() == 0;
        } else if cqe.user_data() == 0x1234 {
            // Timeout should be canceled
            timeout_canceled = cqe.result() == -libc::ECANCELED;
        }
    }

    assert!(cancel_ok, "Cancel operation failed");
    assert!(timeout_canceled, "Timeout was not canceled");

    Ok(())
}

#[test]
fn test_drain_operations() -> Result<()> {
    let mut ring = IoUring::new(8)?;

    // Submit multiple NOPs, with a drain in the middle
    {
        let mut sq = ring.submission();

        // NOP 1
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(1);

        // NOP 2
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(2);

        // NOP 3 with DRAIN (waits for all previous to complete)
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_flags(SqeFlags::IO_DRAIN.bits());
        sqe.set_user_data(3);

        // NOP 4
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(4);
    }

    ring.submit_and_wait(4)?;

    // All should complete
    let mut cq = ring.completion();
    for _ in 0..4 {
        let cqe = cq.wait_cqe()?;
        assert!(cqe.is_success());
    }

    Ok(())
}

#[test]
fn test_multiple_rings() -> Result<()> {
    // Create multiple independent rings
    let mut ring1 = IoUring::new(8)?;
    let mut ring2 = IoUring::new(8)?;

    // Submit to ring1
    {
        let mut sq = ring1.submission();
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(111);
    }

    // Submit to ring2
    {
        let mut sq = ring2.submission();
        let sqe = sq.get_sqe_or_err()?;
        Nop.prepare(sqe);
        sqe.set_user_data(222);
    }

    ring1.submit()?;
    ring2.submit()?;

    // Get completions
    {
        let mut cq1 = ring1.completion();
        let cqe = cq1.wait_cqe()?;
        assert_eq!(cqe.user_data(), 111);
    }

    {
        let mut cq2 = ring2.completion();
        let cqe = cq2.wait_cqe()?;
        assert_eq!(cqe.user_data(), 222);
    }

    Ok(())
}

#[test]
fn test_queue_full() -> Result<()> {
    let mut ring = IoUring::new(4)?; // Small ring

    // Fill the queue
    {
        let mut sq = ring.submission();

        // Fill all 4 slots
        for i in 0..4 {
            let sqe = sq.get_sqe_or_err()?;
            Nop.prepare(sqe);
            sqe.set_user_data(i);
        }

        // Next one should fail
        assert!(sq.get_sqe().is_none(), "Queue should be full");
    }

    // Submit and clear
    ring.submit()?;

    // Now we can get more SQEs
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe();
        assert!(sqe.is_some(), "Should have space after submit");
    }

    Ok(())
}
