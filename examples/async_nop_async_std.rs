//! Simple async NOP example with async-std runtime
//!
//! Run with: cargo run --example async_nop_async_std --features async-async-std

use liburing_rs::async_io::AsyncIoUring;
use liburing_rs::ops::Nop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    async_std::task::block_on(async {
        println!("Creating async io_uring with 8 entries...");
        let mut ring = AsyncIoUring::new(8)?;

        println!("Submitting NOP operation...");
        let result = ring.submit_op(Nop).await?;

        println!("Async NOP completed!");
        println!("  result: {}", result);
        println!("  success: {}", result == 0);

        Ok(())
    })
}
