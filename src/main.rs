use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, TcpListener},
    sync::atomic::{AtomicUsize, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};

const CONNECT_LIMIT: usize = 1;
const BUFFER_SIZE: usize = 128;
static CONNECTS: AtomicUsize = AtomicUsize::new(0);

fn main() -> io::Result<()> {
    if let Ok(listener) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 7)) {
        let mut iter = listener.incoming();
        loop {
            if CONNECTS.load(Ordering::Relaxed) < CONNECT_LIMIT {
                CONNECTS.fetch_add(1, Ordering::Relaxed);
                if let Some(Ok(mut stream)) = iter.next() {
                    spawn(move || {
                        let mut buf = [0; BUFFER_SIZE];
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => match stream.write(&buf[0..n]) {
                                    Ok(0) => break,
                                    Ok(_) => (),
                                    Err(_) => break,
                                },
                                Err(_) => break,
                            }
                        }
                        CONNECTS.fetch_sub(1, Ordering::Relaxed);
                    });
                }
            } else {
                sleep(Duration::from_secs(1));
            }
        }
    }
    Ok(())
}
