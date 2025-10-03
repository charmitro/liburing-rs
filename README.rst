liburing-rs
===========

Rust bindings for liburing (Linux io_uring).

This provides Rust FFI bindings and idiomatic wrappers for https://github.com/axboe/liburing

Requirements
------------

- Linux kernel 5.1+
- **liburing 2.12 or newer** (liburing.so)

Installing liburing
-------------------

**Debian/Ubuntu:**

.. code:: bash

   apt install liburing-dev

**Fedora:**

.. code:: bash

   dnf install liburing-devel

**Arch:**

.. code:: bash

   pacman -S liburing

**From source:**

.. code:: bash

   git clone https://github.com/axboe/liburing.git
   cd liburing
   ./configure
   make
   sudo make install

Build
-----

.. code:: bash

   cargo build --release

The build script:

1. Tries pkg-config to find system liburing
2. Falls back to cloning and building liburing-2.12 from source if not found
3. Uses bindgen to generate FFI bindings

Usage
-----

.. code:: rust

   use liburing_rs::{IoUring, ops::*};

   let mut ring = IoUring::new(32)?;

   // Submit operations
   {
       let mut sq = ring.submission();
       let sqe = sq.get_sqe_or_err()?;
       Nop.prepare(sqe);
       sqe.set_user_data(1);
   }

   ring.submit()?;

   // Get completions
   let mut cq = ring.completion();
   let cqe = cq.wait_cqe()?;
   println!("Result: {}", cqe.result());

Examples
--------

.. code:: bash

   # Basic NOP operation
   cargo run --example nop

   # File copy
   cargo run --release --example io_uring-cp source.txt dest.txt

   # Linked operations
   cargo run --release --example link-cp source.txt dest.txt

   # Polling benchmark
   cargo run --release --example poll-bench

Tests
-----

.. code:: bash

   cargo test --all

Coverage includes:

- Basic operations (NOP, fsync, close)
- File I/O (read, write, readv, writev)
- Network I/O (accept, connect, send, recv)
- Advanced features (timeout, poll, linking, cancellation)

Architecture
------------

Three layers:

1. **sys**: Raw FFI bindings (unsafe)
2. **Safe wrappers**: RAII types (IoUring, SubmissionQueue, CompletionQueue)
3. **Operations**: Type-safe operation builders (Read, Write, etc.)

Performance
-----------

poll-bench achieves ~12M ops/sec (93% of C liburing performance).

License
-------

MIT

Author
------

Charalampos Mitrodimas <charmitro@posteo.net>
