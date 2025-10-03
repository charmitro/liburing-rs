//! Polling benchmark using io_uring
//!
//! This benchmark measures how many poll operations per second can be
//! processed through io_uring. It:
//! - Creates a pipe
//! - Submits multiple POLL_ADD operations on the read end
//! - Writes/reads data to trigger the polls
//! - Measures throughput in requests/second
//!
//! This demonstrates io_uring's polling capabilities and performance.
//!
//! Usage: cargo run --release --example poll-bench

use liburing_rs::{flags::SqeFlags, ops::SqeExt, IoUring};
use std::time::Instant;

const QUEUE_DEPTH: usize = 32;
const RUNTIME_MS: u64 = 10000; // 10 seconds

fn get_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a pipe
    let mut pipe_fds = [0i32; 2];
    let ret = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };
    if ret != 0 {
        return Err("Failed to create pipe".into());
    }

    let (read_fd, write_fd) = (pipe_fds[0], pipe_fds[1]);

    println!("Creating io_uring with queue depth {}...", QUEUE_DEPTH * 32);

    // Try with SINGLE_ISSUER flag first
    let mut ring = match IoUring::with_flags(1024, liburing_rs::flags::SetupFlags::SINGLE_ISSUER) {
        Ok(ring) => {
            println!("Using SINGLE_ISSUER flag");
            ring
        }
        Err(_) => {
            println!("SINGLE_ISSUER not supported, using default flags");
            IoUring::new(1024)?
        }
    };

    // Register the pipe file descriptors as fixed files
    let fds = [read_fd, write_fd];
    unsafe {
        let ret = liburing_rs::sys::io_uring_register_files(
            &mut ring as *mut _ as *mut liburing_rs::sys::io_uring,
            fds.as_ptr(),
            2,
        );
        if ret < 0 {
            eprintln!("Warning: io_uring_register_files failed: {}", -ret);
        } else {
            println!("Registered fixed files");
        }
    }

    // Register the ring fd
    unsafe {
        let ret = liburing_rs::sys::io_uring_register_ring_fd(
            &mut ring as *mut _ as *mut liburing_rs::sys::io_uring,
        );
        if ret < 0 {
            eprintln!("Warning: io_uring_register_ring_fd failed: {}", -ret);
        } else {
            println!("Registered ring fd");
        }
    }

    println!("\nRunning benchmark for {} seconds...", RUNTIME_MS / 1000);
    let start = Instant::now();
    let tstop = get_time_ms() + RUNTIME_MS;
    let mut nr_reqs = 0u64;
    let mut buf = [0u8; 1];

    loop {
        if get_time_ms() >= tstop {
            break;
        }

        // Submit multiple poll operations
        {
            let mut sq = ring.submission();
            for _ in 0..QUEUE_DEPTH {
                if let Some(sqe) = sq.get_sqe() {
                    unsafe {
                        // Poll for POLLIN on read end of pipe (fixed file index 0)
                        liburing_rs::sys::io_uring_prep_poll_add(
                            sqe,
                            0, // Fixed file index
                            libc::POLLIN as u32,
                        );
                    }
                    sqe.set_flags(SqeFlags::FIXED_FILE.bits()); // Use fixed file
                    sqe.set_user_data(1);
                }
            }
        }

        let submitted = ring.submit()?;
        if submitted != QUEUE_DEPTH {
            eprintln!(
                "Warning: only submitted {} out of {}",
                submitted, QUEUE_DEPTH
            );
        }

        // Write 1 byte to trigger the polls
        let ret = unsafe { libc::write(write_fd, buf.as_ptr() as *const _, 1) };
        if ret != 1 {
            return Err("Write failed".into());
        }

        // Read 1 byte back
        let ret = unsafe { libc::read(read_fd, buf.as_mut_ptr() as *mut _, 1) };
        if ret != 1 {
            return Err("Read failed".into());
        }

        // Wait for all poll completions
        {
            let mut cq = ring.completion();
            for _ in 0..QUEUE_DEPTH {
                let cqe = cq.wait_cqe()?;
                if cqe.result() < 0 {
                    eprintln!("Poll failed: {}", cqe.result());
                }
                nr_reqs += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    let requests_per_sec = (nr_reqs * 1000) / RUNTIME_MS;

    println!("\n=== Results ===");
    println!("Total requests: {}", nr_reqs);
    println!("Elapsed time: {:.2}s", elapsed.as_secs_f64());
    println!("Requests/second: {}", requests_per_sec);
    println!(
        "Throughput: {:.2} M ops/sec",
        requests_per_sec as f64 / 1_000_000.0
    );

    // Cleanup
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }

    Ok(())
}
