use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener, UdpSocket},
    sync::atomic::{AtomicUsize, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};

use log::{info, warn, LevelFilter};

const CONNECT_LIMIT: usize = 8;
const BUFFER_SIZE: usize = 128;
const CONNECTION_TIMEOUT: u64 = 30; // seconds

static CONNECTS: AtomicUsize = AtomicUsize::new(0);

fn main() -> io::Result<()> {
    env_logger::Builder::new()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .init();

    spawn(|| -> io::Result<()> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 7))?;
        let mut buf = [0; BUFFER_SIZE];

        loop {
            let (n, remote) = socket.recv_from(&mut buf)?;
            socket.send_to(&buf[0..n], remote)?;
            info!(
                "udp://{} {}",
                remote,
                String::from_utf8_lossy(&buf[0..n]).trim_end_matches(['\r', '\n'].as_ref())
            );
        }
    });

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 7))?;
    let mut iter = listener.incoming();

    loop {
        if CONNECTS.load(Ordering::Relaxed) < CONNECT_LIMIT {
            CONNECTS.fetch_add(1, Ordering::Relaxed);

            if let Some(Ok(mut stream)) = iter.next() {
                let remote = stream.peer_addr()?;
                info!("{} Establish a connection", remote);

                spawn(move || -> io::Result<()> {
                    let mut buf = [0; BUFFER_SIZE];

                    stream.set_read_timeout(Some(Duration::from_secs(CONNECTION_TIMEOUT)))?;

                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => match stream.write(&buf[0..n]) {
                                Ok(0) => break,
                                Ok(_) => info!(
                                    "tcp://{} {}",
                                    remote,
                                    String::from_utf8_lossy(&buf[0..n])
                                        .trim_end_matches(['\r', '\n'].as_ref())
                                ),
                                Err(_) => break,
                            },
                            Err(e) => {
                                if e.kind() == ErrorKind::WouldBlock {
                                    warn!("{} Connection timeout", remote);
                                }
                                break;
                            }
                        }
                    }

                    CONNECTS.fetch_sub(1, Ordering::Relaxed);
                    info!("{} Disconnected", remote);

                    Ok(())
                });
            }
        } else {
            sleep(Duration::from_secs(1));
        }
    }
}
