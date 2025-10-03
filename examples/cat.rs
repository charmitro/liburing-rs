//! Simple cat implementation using io_uring
//!
//! This example demonstrates basic sequential file reading with io_uring.
//! It reads a file in chunks and prints it to stdout.
//!
//! Usage: cargo run --example cat <filename>

use liburing_rs::{ops::*, IoUring, Result};
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;

const BLOCK_SIZE: usize = 4096;

fn cat_file(file: &File, ring: &mut IoUring) -> Result<()> {
    let fd = file.as_raw_fd();
    let mut offset = 0u64;
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    loop {
        let mut buffer = vec![0u8; BLOCK_SIZE];

        // Submit read operation
        {
            let mut sq = ring.submission();
            let sqe = sq.get_sqe_or_err()?;
            Read::from_slice(fd, &mut buffer, offset).prepare(sqe);
            sqe.set_user_data(1);
        }

        ring.submit_and_wait(1)?;

        // Get completion
        let bytes_read = {
            let mut cq = ring.completion();
            let cqe = cq.wait_cqe()?;
            let result = cqe.result();

            if result < 0 {
                return Err(liburing_rs::Error::Io(std::io::Error::from_raw_os_error(
                    -result,
                )));
            }

            result as usize
        };

        if bytes_read == 0 {
            break; // EOF
        }

        // Write to stdout
        handle.write_all(&buffer[..bytes_read])?;
        offset += bytes_read as u64;
    }

    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let file = File::open(&args[1])?;
    let mut ring = IoUring::new(8)?;

    cat_file(&file, &mut ring)?;

    Ok(())
}
