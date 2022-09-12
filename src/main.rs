use log::{info, warn, LevelFilter};
use std::{
    io::{self, ErrorKind},
    net::Ipv4Addr,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    runtime::Builder,
    signal::ctrl_c,
    task::{self, JoinHandle},
};

const CONNECT_LIMIT: usize = 8;
const BUFFER_SIZE: usize = 128;

static CONNECTS: AtomicUsize = AtomicUsize::new(0);

fn main() -> io::Result<()> {
    env_logger::Builder::new()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .init();

    let rt = Builder::new_current_thread().enable_all().build()?;

    rt.block_on(async {
        let udp_worker: JoinHandle<io::Result<()>> = task::spawn(async {
            let mut buf = [0; BUFFER_SIZE];
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 7)).await?;

            loop {
                let (rxn, remote) = socket.recv_from(&mut buf).await?;
                let txn = socket.send_to(&buf[0..rxn], remote).await?;

                debug_assert!(rxn == txn);
                info!(
                    "udp://{remote} {}",
                    String::from_utf8_lossy(&buf[0..rxn]).trim_end_matches(['\r', '\n'].as_ref())
                );
            }
        });

        let tcp_worker: JoinHandle<io::Result<()>> = task::spawn(async {
            let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 7)).await?;

            loop {
                let (mut stream, _addr) = listener.accept().await?;

                if CONNECTS.load(Ordering::Relaxed) < CONNECT_LIMIT {
                    CONNECTS.fetch_add(1, Ordering::Relaxed);

                    let _: JoinHandle<io::Result<()>> = task::spawn(async move {
                        let mut buf = [0; BUFFER_SIZE];

                        let remote = stream.peer_addr()?;
                        info!("{remote} Establish a connection");

                        loop {
                            match stream.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => match stream.write(&buf[0..n]).await {
                                    Ok(0) => break,
                                    Ok(_) => info!(
                                        "tcp://{remote} {}",
                                        String::from_utf8_lossy(&buf[0..n])
                                            .trim_end_matches(['\r', '\n'].as_ref())
                                    ),
                                    Err(_) => break,
                                },
                                Err(e) => {
                                    if e.kind() == ErrorKind::WouldBlock {
                                        warn!("{remote} Connection timeout");
                                    }
                                    break;
                                }
                            }
                        }

                        CONNECTS.fetch_sub(1, Ordering::Relaxed);
                        info!("{remote} Disconnected");

                        Ok(())
                    });
                } else {
                    task::yield_now().await;
                }
            }
        });

        info!("Server start");

        loop {
            tokio::select! {
                _ = udp_worker => {
                    break;
                }
                _ = tcp_worker => {
                    break;
                }
                _ = ctrl_c() => {
                    break;
                }
            }
        }
    });

    rt.shutdown_background();
    info!("Server stop");

    Ok(())
}
