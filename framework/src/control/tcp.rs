use super::{Available, PollScheduler, Token, HUP, READ, WRITE};
use fnv::FnvHasher;
/// TCP connection.
use net2::TcpBuilder;
use scheduler::Executable;

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::marker::PhantomData;
use std::net::*;

pub trait TcpControlAgent {
    fn new(address: SocketAddr, stream: TcpStream) -> Self;
    fn handle_read_ready(&mut self) -> bool;
    fn handle_write_ready(&mut self) -> bool;
    fn handle_hup(&mut self) -> bool;
}

type FnvHash = BuildHasherDefault<FnvHasher>;
pub struct TcpControlServer<T: TcpControlAgent> {
    listener: TcpListener,
    scheduler: PollScheduler,
    next_token: Token,
    listener_token: Token,
    phantom_t: PhantomData<T>,
    connections: HashMap<Token, T, FnvHash>,
}

impl<T: TcpControlAgent> Executable for TcpControlServer<T> {
    fn execute(&mut self) -> (u32, i32) {
        self.schedule();
        (1u32, 0)
    }

    //    #[inline]
    //    fn dependencies(&mut self) -> Vec<usize> {
    //        vec![]
    //    }
}

impl<T: TcpControlAgent> TcpControlServer<T> {
    pub fn new(address: SocketAddr) -> TcpControlServer<T> {
        let socket = match address {
            SocketAddr::V4(_) => TcpBuilder::new_v4(),
            SocketAddr::V6(_) => TcpBuilder::new_v6(),
        }
        .unwrap();
        let _ = socket.reuse_address(true).unwrap();
        // TODO: Change 1024 to a parameter
        let listener = socket.bind(address).unwrap().listen(1024).unwrap();
        listener.set_nonblocking(true).unwrap();
        let scheduler = PollScheduler::new();
        let listener_token = 0;
        let handle = scheduler.poll_handle();
        handle.new_io_port(&listener, listener_token);
        handle.schedule_read(&listener, listener_token);
        TcpControlServer {
            listener,
            scheduler,
            next_token: listener_token + 1,
            listener_token,
            phantom_t: PhantomData,
            connections: HashMap::with_capacity_and_hasher(32, Default::default()),
        }
    }

    pub fn schedule(&mut self) {
        match self.scheduler.get_token_noblock() {
            Some((token, avail)) if token == self.listener_token => {
                self.accept_connection(avail);
            }
            Some((token, available)) => {
                self.handle_data(token, available);
            }
            _ => {}
        }
    }

    #[cfg_attr(feature = "dev", allow(single_match))]
    fn accept_connection(&mut self, available: Available) {
        if available & READ != 0 {
            // Make sure we have something to accept
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    let token = self.next_token;
                    self.next_token += 1;
                    stream.set_nonblocking(true).unwrap();
                    //let stream_fd = stream.as_fd();
                    //let scheduler = IOScheduler::new(self.scheduler.poll_handle(), stream_fd.clone(), token);
                    self.connections.insert(token, T::new(addr, stream));
                    // Add to some sort of hashmap.
                }
                Err(_) => {
                    // TODO: Record
                }
            }
        } else {
            // TODO: Report something.
        }
        self.scheduler
            .poll_handle()
            .schedule_read(&self.listener, self.listener_token);
    }

    fn handle_data(&mut self, token: Token, available: Available) {
        let preserve = {
            match self.connections.get_mut(&token) {
                Some(connection) => {
                    if available & READ != 0 {
                        connection.handle_read_ready()
                    } else if available & WRITE != 0 {
                        connection.handle_write_ready()
                    } else if available & HUP != 0 {
                        connection.handle_hup()
                    } else {
                        true
                    }
                }
                None => {
                    // TODO: Record
                    true
                }
            }
        };

        if !preserve {
            self.connections.remove(&token);
        }
    }
}
