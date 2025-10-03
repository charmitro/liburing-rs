//! Simple async NOP example with tokio runtime
//!
//! Run with: cargo run --example async_nop_tokio --features async-tokio

use liburing_rs::async_io::AsyncIoUring;
use liburing_rs::ops::Nop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating async io_uring with 8 entries...");
    let mut ring = AsyncIoUring::new(8)?;

    println!("Submitting NOP operation...");
    let result = ring.submit_op(Nop).await?;

    println!("Async NOP completed!");
    println!("  result: {}", result);
    println!("  success: {}", result == 0);

    Ok(())
}
