//! Network I/O operation tests
//! Corresponds to liburing tests: accept.c, connect.c, send.c, recv.c, sendmsg.c, recvmsg.c

use liburing_rs::{ops::*, IoUring, Result};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::thread;
use std::time::Duration;

#[test]
fn test_accept() -> Result<()> {
    // Create a listening socket
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let listener_fd = listener.as_raw_fd();

    // Spawn a client thread
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let _ = TcpStream::connect(addr);
    });

    let mut ring = IoUring::new(8)?;

    // Submit accept operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            Accept::new(listener_fd, std::ptr::null_mut(), std::ptr::null_mut(), 0).prepare(sqe);
        }
        sqe.set_user_data(1);
    }

    ring.submit()?;

    // Wait for connection
    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;
    let client_fd = cqe.result();

    assert!(client_fd > 0, "Accept failed: {}", client_fd);

    // Clean up
    unsafe {
        libc::close(client_fd);
    }

    Ok(())
}

#[test]
fn test_connect() -> Result<()> {
    // Create a listening socket
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn accept thread
    thread::spawn(move || {
        let _ = listener.accept();
    });

    thread::sleep(Duration::from_millis(50));

    // Create a socket for connecting
    let socket = unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
        assert!(fd >= 0);
        fd
    };

    // Set non-blocking
    unsafe {
        let flags = libc::fcntl(socket, libc::F_GETFL, 0);
        libc::fcntl(socket, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let mut ring = IoUring::new(8)?;

    // Prepare sockaddr
    let sockaddr = match addr {
        SocketAddr::V4(addr) => {
            let sin = libc::sockaddr_in {
                sin_family: libc::AF_INET as u16,
                sin_port: addr.port().to_be(),
                sin_addr: libc::in_addr {
                    s_addr: u32::from_ne_bytes(addr.ip().octets()),
                },
                sin_zero: [0; 8],
            };
            sin
        }
        _ => panic!("Expected IPv4 address"),
    };

    // Submit connect operation
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            Connect::new(
                socket,
                &sockaddr as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as u32,
            )
            .prepare(sqe);
        }
        sqe.set_user_data(1);
    }

    ring.submit_and_wait(1)?;

    let mut cq = ring.completion();
    let cqe = cq.wait_cqe()?;

    // Connect result: 0 or -EINPROGRESS is success
    assert!(cqe.result() == 0 || cqe.result() == -libc::EINPROGRESS);

    unsafe {
        libc::close(socket);
    }

    Ok(())
}

#[test]
fn test_send_recv() -> Result<()> {
    // Create a socket pair
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(ret, 0);

    let (sock1, sock2) = (fds[0], fds[1]);

    let mut ring = IoUring::new(8)?;

    // Send data
    let send_data = b"Hello from io_uring!";
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_send(
                sqe,
                sock1,
                send_data.as_ptr() as *const _,
                send_data.len(),
                0,
            );
        }
        sqe.set_user_data(1);
    }

    ring.submit()?;

    // Wait for send completion
    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, send_data.len());
    }

    // Receive data
    let mut recv_buf = vec![0u8; send_data.len()];
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_recv(
                sqe,
                sock2,
                recv_buf.as_mut_ptr() as *mut _,
                recv_buf.len(),
                0,
            );
        }
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;

    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, send_data.len());
    }

    assert_eq!(&recv_buf[..], send_data);

    unsafe {
        libc::close(sock1);
        libc::close(sock2);
    }

    Ok(())
}

#[test]
fn test_sendmsg_recvmsg() -> Result<()> {
    // Create a socket pair
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(ret, 0);

    let (sock1, sock2) = (fds[0], fds[1]);

    let mut ring = IoUring::new(8)?;

    // Prepare message to send
    let send_data = b"Test message via sendmsg/recvmsg";
    let iov = libc::iovec {
        iov_base: send_data.as_ptr() as *mut _,
        iov_len: send_data.len(),
    };

    let msg = libc::msghdr {
        msg_name: std::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &iov as *const _ as *mut _,
        msg_iovlen: 1,
        msg_control: std::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    };

    // Send message
    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_sendmsg(sqe, sock1, &msg, 0);
        }
        sqe.set_user_data(1);
    }

    ring.submit()?;

    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, send_data.len());
    }

    // Receive message
    let mut recv_buf = vec![0u8; send_data.len()];
    let recv_iov = libc::iovec {
        iov_base: recv_buf.as_mut_ptr() as *mut _,
        iov_len: recv_buf.len(),
    };

    let mut recv_msg = libc::msghdr {
        msg_name: std::ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: &recv_iov as *const _ as *mut _,
        msg_iovlen: 1,
        msg_control: std::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    };

    {
        let mut sq = ring.submission();
        let sqe = sq.get_sqe_or_err()?;
        unsafe {
            liburing_rs::sys::io_uring_prep_recvmsg(sqe, sock2, &mut recv_msg, 0);
        }
        sqe.set_user_data(2);
    }

    ring.submit_and_wait(1)?;

    {
        let mut cq = ring.completion();
        let cqe = cq.wait_cqe()?;
        assert_eq!(cqe.result() as usize, send_data.len());
    }

    assert_eq!(&recv_buf[..], send_data);

    unsafe {
        libc::close(sock1);
        libc::close(sock2);
    }

    Ok(())
}

#[test]
fn test_multiple_send_recv() -> Result<()> {
    // Use DGRAM (UDP) sockets to preserve message boundaries (like liburing test)
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_DGRAM, 0, fds.as_mut_ptr()) };
    assert_eq!(ret, 0);

    let (sock1, sock2) = (fds[0], fds[1]);

    let mut ring = IoUring::new(16)?;

    // Send multiple messages
    const NUM_MSGS: usize = 5;
    let messages: Vec<Vec<u8>> = (0..NUM_MSGS)
        .map(|i| format!("Message {}", i).into_bytes())
        .collect();

    {
        let mut sq = ring.submission();
        for (i, msg) in messages.iter().enumerate() {
            let sqe = sq.get_sqe_or_err()?;
            unsafe {
                liburing_rs::sys::io_uring_prep_send(
                    sqe,
                    sock1,
                    msg.as_ptr() as *const _,
                    msg.len(),
                    0,
                );
            }
            sqe.set_user_data(i as u64);
        }
    }

    ring.submit()?;

    // Wait for all sends
    {
        let mut cq = ring.completion();
        for _ in 0..NUM_MSGS {
            let cqe = cq.wait_cqe()?;
            assert!(cqe.result() > 0);
        }
    }

    // Receive all messages
    let mut recv_buffers: Vec<Vec<u8>> = (0..NUM_MSGS).map(|_| vec![0u8; 128]).collect();

    {
        let mut sq = ring.submission();
        for (i, buf) in recv_buffers.iter_mut().enumerate() {
            let sqe = sq.get_sqe_or_err()?;
            unsafe {
                liburing_rs::sys::io_uring_prep_recv(
                    sqe,
                    sock2,
                    buf.as_mut_ptr() as *mut _,
                    buf.len(),
                    0,
                );
            }
            sqe.set_user_data((NUM_MSGS + i) as u64);
        }
    }

    ring.submit_and_wait(NUM_MSGS as u32)?;

    {
        let mut cq = ring.completion();
        for _ in 0..NUM_MSGS {
            let cqe = cq.wait_cqe()?;
            assert!(cqe.result() > 0);
        }
    }

    // Verify messages
    for (i, (sent, recv)) in messages.iter().zip(recv_buffers.iter()).enumerate() {
        let recv_len = sent.len();
        assert_eq!(&recv[..recv_len], &sent[..], "Message {} mismatch", i);
    }

    unsafe {
        libc::close(sock1);
        libc::close(sock2);
    }

    Ok(())
}
