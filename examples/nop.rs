//! Simple example demonstrating basic io_uring usage with NOP operations
//!
//! This example creates an io_uring instance, submits a NOP (no operation) request,
//! and waits for completion. This is the simplest possible io_uring program.

use liburing_rs::{
    ops::{Nop, PrepareOp, SqeExt},
    IoUring,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating io_uring with 8 entries...");
    let mut ring = IoUring::new(8)?;

    // Get a submission queue entry
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;

        // Prepare a NOP operation
        let nop = Nop;
        nop.prepare(sqe);
        sqe.set_user_data(0x1234);
    }

    println!("Submitting NOP operation...");
    ring.submit()?;

    // Wait for completion
    println!("Waiting for completion...");
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;

    println!("Completion received!");
    println!("  user_data: 0x{:x}", cqe.user_data());
    println!("  result: {}", cqe.result());
    println!("  success: {}", cqe.is_success());

    Ok(())
}
