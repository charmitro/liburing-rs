//! File copy using linked io_uring operations
//!
//! This example demonstrates io_uring's IOSQE_IO_LINK flag by linking
//! read and write operations. When operations are linked, the write will
//! only execute if the read succeeds. This provides automatic error handling
//! and ensures operations happen in order.
//!
//! Usage: cargo run --example link-cp <source> <destination>

use liburing_rs::{flags::SqeFlags, ops::*, IoUring, Result};
use std::env;
use std::fs::{metadata, File, OpenOptions};
use std::os::unix::io::{AsRawFd, RawFd};

const QUEUE_DEPTH: usize = 64;
const BLOCK_SIZE: usize = 32 * 1024; // 32KB

struct IoData {
    buffer: Vec<u8>,
    #[allow(dead_code)]
    offset: u64,
    completed_ops: usize, // Track read(0) and write(1) completion
}

fn copy_file_linked(infd: RawFd, outfd: RawFd, file_size: u64, ring: &mut IoUring) -> Result<()> {
    let mut offset = 0u64;
    let mut operations: Vec<IoData> = Vec::new();
    let mut inflight = 0usize;

    while offset < file_size || inflight > 0 {
        // Queue read-write pairs with linking
        while offset < file_size && inflight < QUEUE_DEPTH {
            let size = std::cmp::min((file_size - offset) as usize, BLOCK_SIZE);
            let mut buffer = vec![0u8; size];
            let op_offset = offset;
            let op_index = operations.len();

            {
                let mut sq = ring.submission();

                // Queue read operation with LINK flag
                if let Some(sqe) = sq.get_sqe() {
                    Read::from_slice(infd, &mut buffer, op_offset).prepare(sqe);
                    sqe.set_flags(SqeFlags::IO_LINK.bits());
                    sqe.set_user_data((op_index * 2) as u64); // Even = read
                } else {
                    break;
                }

                // Queue linked write operation
                // This will only execute if the read succeeds
                if let Some(sqe) = sq.get_sqe() {
                    Write::from_slice(outfd, &buffer, op_offset).prepare(sqe);
                    sqe.set_user_data((op_index * 2 + 1) as u64); // Odd = write
                } else {
                    break;
                }
            }

            operations.push(IoData {
                buffer,
                offset: op_offset,
                completed_ops: 0,
            });

            offset += size as u64;
            inflight += 2; // Read + write
        }

        if inflight > 0 {
            ring.submit()?;
        }

        // Process completions
        {
            let mut cq = ring.completion();
            while let Some(cqe) = cq.peek_cqe() {
                let result = cqe.result();
                let user_data = cqe.user_data() as usize;
                let op_index = user_data / 2;
                let is_read = (user_data % 2) == 0;

                if result < 0 {
                    // If read fails, write is automatically canceled with -ECANCELED
                    if result == -libc::ECANCELED {
                        eprintln!("Linked operation canceled (previous operation failed)");
                    } else {
                        return Err(liburing_rs::Error::Io(std::io::Error::from_raw_os_error(
                            -result,
                        )));
                    }
                }

                if let Some(op) = operations.get_mut(op_index) {
                    op.completed_ops += 1;
                    if is_read && result > 0 {
                        // Adjust write size if short read
                        op.buffer.truncate(result as usize);
                    }
                }

                inflight -= 1;
            }
        }
    }

    println!("All operations completed successfully");
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
        "Copying {} bytes from {} to {} using linked operations",
        file_size, args[1], args[2]
    );

    let mut ring = IoUring::new(QUEUE_DEPTH as u32)?;
    copy_file_linked(infd, outfd, file_size, &mut ring)?;

    println!("Copy completed successfully!");
    Ok(())
}
