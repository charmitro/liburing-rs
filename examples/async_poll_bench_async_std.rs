//! Async polling benchmark using io_uring with async-std
//!
//! This benchmark measures how many poll operations per second can be
//! processed through io_uring using the async API. It:
//! - Creates a pipe
//! - Submits POLL_ADD operations using async/await
//! - Measures throughput in requests/second
//!
//! This demonstrates io_uring's async polling capabilities with async-std.
//!
//! Usage: cargo run --release --example async_poll_bench_async_std --features async-async-std

use liburing_rs::async_io::async_std_impl::AsyncIoUring;
use liburing_rs::ops::PrepareOp;
use std::time::Instant;

const BATCH_SIZE: usize = 32;
const RUNTIME_MS: u64 = 10000; // 10 seconds

struct PollOp {
    fd: i32,
    poll_mask: u32,
}

impl PrepareOp for PollOp {
    fn prepare(&self, sqe: &mut liburing_rs::sys::io_uring_sqe) {
        unsafe {
            liburing_rs::sys::io_uring_prep_poll_add(sqe, self.fd, self.poll_mask);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    async_std::task::block_on(async {
        // Create a pipe
        let mut pipe_fds = [0i32; 2];
        let ret = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };
        if ret != 0 {
            return Err("Failed to create pipe".into());
        }

        let (read_fd, write_fd) = (pipe_fds[0], pipe_fds[1]);

        println!("Creating async io_uring with batch size {}...", BATCH_SIZE);

        let mut ring = AsyncIoUring::new(256)?;

        println!(
            "\nRunning async benchmark for {} seconds...",
            RUNTIME_MS / 1000
        );
        let start = Instant::now();

        let mut nr_reqs = 0u64;
        let buf = [0u8; 1];

        loop {
            if start.elapsed().as_millis() as u64 >= RUNTIME_MS {
                break;
            }

            // Submit a batch of poll operations sequentially
            for _ in 0..BATCH_SIZE {
                // Write 1 byte to trigger the poll
                let ret = unsafe { libc::write(write_fd, buf.as_ptr() as *const _, 1) };
                if ret != 1 {
                    return Err("Write failed".into());
                }

                // Submit poll operation and await completion
                match ring
                    .submit_op(PollOp {
                        fd: read_fd,
                        poll_mask: libc::POLLIN as u32,
                    })
                    .await
                {
                    Ok(result) => {
                        if result >= 0 {
                            nr_reqs += 1;
                        } else {
                            eprintln!("Poll failed: {}", result);
                        }
                    }
                    Err(e) => {
                        eprintln!("Poll error: {:?}", e);
                    }
                }

                // Read 1 byte back
                let ret = unsafe { libc::read(read_fd, buf.as_ptr() as *mut _, 1) };
                if ret != 1 {
                    return Err("Read failed".into());
                }
            }
        }

        let elapsed = start.elapsed();
        let requests_per_sec = (nr_reqs * 1000) / RUNTIME_MS;

        println!("\n=== Async Results ===");
        println!("Total requests: {}", nr_reqs);
        println!("Elapsed time: {:.2}s", elapsed.as_secs_f64());
        println!("Requests/second: {}", requests_per_sec);
        println!(
            "Throughput: {:.2} K ops/sec",
            requests_per_sec as f64 / 1_000.0
        );

        // Cleanup
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }

        Ok(())
    })
}
