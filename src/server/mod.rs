pub mod command;
pub mod db;

use command::{ArkeCommand, CommandHandler};
use log::{debug, error, info};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_rustls::{rustls, server::TlsStream, TlsAcceptor};

pub struct ArkeServer {
    listener: TcpListener,
    certs: Vec<rustls::Certificate>,
    private_key: rustls::PrivateKey,
    handlers: HashMap<u8, Box<dyn CommandHandler>>,
}

impl ArkeServer {
    pub fn builder() -> ArkeServerBuilder {
        ArkeServerBuilder {
            bind_port: 8080,
            bind_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            certs: vec![],
            private_key: None,
            handlers: None,
        }
    }

    pub async fn new(
        bind_port: u16,
        bind_addr: IpAddr,
        certs: Vec<rustls::Certificate>,
        private_key: rustls::PrivateKey,
        handlers: HashMap<u8, Box<dyn CommandHandler>>,
    ) -> Result<Self, tokio::io::Error> {
        let bind_addr = format!("{}:{}", bind_addr, bind_port);
        info!("Server will listen on tcp://{bind_addr}");
        Ok(Self {
            listener: TcpListener::bind(bind_addr).await?,
            certs,
            private_key,
            handlers,
        })
    }

    async fn handle_connection(
        stream: TcpStream,
        acceptor: TlsAcceptor,
        handlers: Arc<Mutex<HashMap<u8, Box<dyn CommandHandler>>>>,
    ) -> Result<(), tokio::io::Error> {
        let peer_addr = stream.peer_addr()?;
        let mut stream = acceptor.accept(stream).await?;

        'connection: loop {
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).await?;

            match serde_json::from_slice::<ArkeCommand>(&buffer[..n]) {
                Ok(command) => {
                    debug!(
                        "Received command with discriminant: {}",
                        command.discriminant()
                    );

                    let mut handlers = handlers.lock().await;
                    let handler = handlers.get_mut(&command.discriminant());
                    let result = handler
                        .expect("Expected command handler to be present")
                        .handle(command)
                        .await;

                    if let ArkeCommand::Goodbye(err) = result {
                        log::info!("Sending Goodbye(Error = {err:?}) for connection {peer_addr}");
                        break 'connection;
                    } else {
                        Self::send_command(&mut stream, result).await?;
                    }
                }
                Err(err) => {
                    error!("Invalid command. {err:?}");
                    Self::send_command(&mut stream, ArkeCommand::Goodbye(None)).await?;
                    break 'connection;
                }
            }
        }

        stream.shutdown().await?;

        info!("Closing connection from {}", peer_addr);
        Ok(())
    }

    async fn send_command(
        stream: &mut TlsStream<TcpStream>,
        command: ArkeCommand,
    ) -> Result<usize, tokio::io::Error> {
        let mut msg = serde_json::to_vec(&command).expect("Couldn't serialize message");
        msg.push("\n".as_bytes()[0]);
        debug!("Sending command: {command:?}");
        stream.write(&msg).await
    }

    pub async fn start(self) -> Result<(), tokio::io::Error> {
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(self.certs, self.private_key)
                .expect("Couldn't create TLS config"),
        );

        let acceptor = TlsAcceptor::from(Arc::clone(&config));

        info!("Starting Arke server...");
        let handlers = Arc::new(Mutex::new(self.handlers));
        loop {
            let acceptor = acceptor.clone();
            let (socket, peer_addr) = self.listener.accept().await?;
            info!("Accepting socket connection from {peer_addr}");
            let handler = Arc::clone(&handlers);
            tokio::spawn(async move { Self::handle_connection(socket, acceptor, handler).await });
        }
    }
}

use std::collections::HashMap;
pub struct ArkeServerBuilder {
    bind_port: u16,
    bind_addr: IpAddr,
    certs: Vec<rustls::Certificate>,
    private_key: Option<rustls::PrivateKey>,
    handlers: Option<HashMap<u8, Box<dyn CommandHandler>>>,
}

impl ArkeServerBuilder {
    pub fn with_certs(mut self, certs: Vec<rustls::Certificate>) -> Self {
        self.certs = certs;
        self
    }

    pub fn with_private_key(mut self, private_key: rustls::PrivateKey) -> Self {
        self.private_key = Some(private_key);
        self
    }

    pub fn with_bind_addr(mut self, bind_addr: IpAddr) -> Self {
        self.bind_addr = bind_addr;
        self
    }

    pub fn with_bind_port(mut self, bind_port: u16) -> Self {
        self.bind_port = bind_port;
        self
    }

    pub async fn build(self) -> Result<ArkeServer, tokio::io::Error> {
        Ok(ArkeServer::new(
            self.bind_port,
            self.bind_addr,
            self.certs,
            self.private_key.unwrap(),
            self.handlers.unwrap(),
        )
        .await?)
    }

    pub fn handlers(mut self, handlers: HashMap<u8, Box<dyn CommandHandler>>) -> Self {
        self.handlers = Some(handlers);
        self
    }
}
