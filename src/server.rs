#![allow(dead_code)]

use std::net::{IpAddr, TcpListener, TcpStream};

pub struct ArkeServer {
    listener: TcpListener,
}

impl ArkeServer {
    pub fn new(config: ArkeServerConfig) -> Result<Self, std::io::Error> {
        Ok(Self {
            listener: TcpListener::bind(format!("{}:{}", config.bind_addr, config.bind_port))?,
        })
    }

    fn handle_connection(_: TcpStream) {
        todo!();
    }

    pub fn start(self) -> Result<(), std::io::Error> {
        loop {
            let (socket, _) = self.listener.accept()?;
            std::thread::spawn(|| Self::handle_connection(socket));
        }
    }
}

pub struct ArkeServerConfig {
    pub bind_port: u16,
    pub bind_addr: IpAddr,
}

impl Default for ArkeServerConfig {
    fn default() -> Self {
        todo!()
    }
}
