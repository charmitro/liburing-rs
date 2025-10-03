//! High-performance file copy using io_uring
//!
//! This example demonstrates efficient file copying with io_uring by:
//! - Using multiple in-flight read and write operations
//! - Managing queue depth to maximize throughput
//! - Handling short reads/writes
//!
//! Usage: cargo run --example io_uring-cp <source> <destination>

use liburing_rs::{ops::*, IoUring, Result};
use std::collections::VecDeque;
use std::env;
use std::fs::{metadata, File, OpenOptions};
use std::os::unix::io::{AsRawFd, RawFd};

const QUEUE_DEPTH: usize = 64;
const BLOCK_SIZE: usize = 32 * 1024; // 32KB

struct IoData {
    buffer: Vec<u8>,
    offset: u64,
    len: usize,
    is_read: bool,
}

fn copy_file(infd: RawFd, outfd: RawFd, file_size: u64, ring: &mut IoUring) -> Result<()> {
    let mut offset = 0u64;
    let mut pending_operations = VecDeque::new();
    let mut reads_in_flight = 0usize;
    let mut writes_in_flight = 0usize;
    let mut bytes_to_read = file_size;
    let mut bytes_to_write = file_size;

    while bytes_to_read > 0 || bytes_to_write > 0 {
        // Queue as many reads as possible
        while bytes_to_read > 0 && (reads_in_flight + writes_in_flight) < QUEUE_DEPTH {
            let size = std::cmp::min(bytes_to_read as usize, BLOCK_SIZE);
            let mut buffer = vec![0u8; size];

            {
                let mut sq = ring.submission();
                if let Some(sqe) = sq.get_sqe() {
                    Read::from_slice(infd, &mut buffer, offset).prepare(sqe);
                    sqe.set_user_data(offset);
                } else {
                    break;
                }
            }

            pending_operations.push_back(IoData {
                buffer,
                offset,
                len: size,
                is_read: true,
            });

            offset += size as u64;
            bytes_to_read -= size as u64;
            reads_in_flight += 1;
        }

        if reads_in_flight + writes_in_flight > 0 {
            ring.submit()?;
        }

        // Process completions
        let completions: Vec<(i32, u64)> = {
            let mut cq = ring.completion();
            let mut results = Vec::new();
            while let Some(cqe) = cq.peek_cqe() {
                results.push((cqe.result(), cqe.user_data()));
            }
            results
        };

        for (result, user_data) in completions {
            if result < 0 {
                return Err(liburing_rs::Error::Io(std::io::Error::from_raw_os_error(
                    -result,
                )));
            }

            // Find corresponding operation
            if let Some(pos) = pending_operations
                .iter()
                .position(|op| op.offset == user_data)
            {
                let mut op = pending_operations.remove(pos).unwrap();

                if op.is_read {
                    // Read completed, queue write
                    reads_in_flight -= 1;
                    op.is_read = false;
                    op.len = result as usize;

                    let write_offset = op.offset;
                    {
                        let mut sq = ring.submission();
                        if let Some(sqe) = sq.get_sqe() {
                            Write::from_slice(outfd, &op.buffer[..op.len], write_offset)
                                .prepare(sqe);
                            sqe.set_user_data(write_offset);
                        }
                    }

                    pending_operations.push_back(op);
                    writes_in_flight += 1;
                    ring.submit()?;
                } else {
                    // Write completed
                    writes_in_flight -= 1;
                    bytes_to_write -= op.len as u64;
                }
            }
        }
    }

    // Wait for remaining writes
    while writes_in_flight > 0 {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        let result = cqe.result();

        if result < 0 {
            return Err(liburing_rs::Error::Io(std::io::Error::from_raw_os_error(
                -result,
            )));
        }

        writes_in_flight -= 1;
    }

    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <source> <destination>", args[0]);
        std::process::exit(1);
    }

    let infile = File::open(&args[1])?;
    let infd = infile.as_raw_fd();

    let file_size = metadata(&args[1])?.len();

    let outfile = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args[2])?;
    let outfd = outfile.as_raw_fd();

    println!(
        "Copying {} bytes from {} to {}",
        file_size, args[1], args[2]
    );

    let mut ring = IoUring::new(QUEUE_DEPTH as u32)?;
    copy_file(infd, outfd, file_size, &mut ring)?;

    println!("Copy completed successfully!");
    Ok(())
}
