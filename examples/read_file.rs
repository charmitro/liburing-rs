//! Example demonstrating file reading with io_uring
//!
//! This example opens a file, reads its contents using io_uring, and prints the result.

use liburing_rs::{
    ops::{PrepareOp, Read, SqeExt},
    IoUring,
};
use std::fs::File;
use std::os::unix::io::AsRawFd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary file with some content
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), b"Hello from io_uring!")?;

    println!("Reading file: {}", tmp.path().display());

    // Open the file
    let file = File::open(tmp.path())?;
    let fd = file.as_raw_fd();

    // Create io_uring instance
    let mut ring = IoUring::new(8)?;

    // Prepare a buffer to read into
    let mut buffer = vec![0u8; 1024];

    // Get a submission queue entry and prepare read operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;

        let read_op = Read::from_slice(fd, &mut buffer, 0);
        read_op.prepare(sqe);
        sqe.set_user_data(0x42);
    }

    // Submit and wait
    println!("Submitting read operation...");
    ring.submit_and_wait(1)?;

    // Get the completion
    let mut cq = ring.completion();
    if let Some(cqe) = cq.peek_cqe() {
        let bytes_read = cqe.result();
        println!("Read {} bytes", bytes_read);

        if bytes_read > 0 {
            let content = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
            println!("Content: {}", content);
        }
    }

    Ok(())
}
