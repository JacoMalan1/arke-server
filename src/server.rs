use async_trait::async_trait;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
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

pub struct ArkeServer<S: Clone> {
    listener: TcpListener,
    certs: Vec<rustls::Certificate>,
    private_key: rustls::PrivateKey,
    handlers: HashMap<u32, Box<dyn ConversationHandler<S>>>,
}

impl<S: Clone> ArkeServer<S> {
    pub async fn new(config: ArkeServerConfig<S>) -> Result<Self, std::io::Error> {
        info!(
            "Arke server will bind to {}:{}",
            config.bind_addr, config.bind_port
        );

        Ok(Self {
            listener: TcpListener::bind(format!("{}:{}", config.bind_addr, config.bind_port))
                .await?,
            certs: config.certs,
            private_key: config.private_key,
            handlers: config.handlers,
        })
    }

    async fn handle_connection(
        stream: TcpStream,
        acceptor: TlsAcceptor,
        handlers: Arc<Mutex<HashMap<u32, Box<dyn ConversationHandler<S>>>>>,
    ) -> Result<(), tokio::io::Error> {
        let peer_addr = stream.peer_addr()?;
        let mut stream = acceptor.accept(stream).await?;

        'connection: loop {
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).await?;

            match serde_json::from_slice::<ArkeCommand>(&buffer[..n]) {
                Ok(command) => {
                    debug!("Received command: {command:?}");

                    let mut handlers = handlers.lock().await;
                    let handler = handlers.get_mut(&command.discriminant());
                    let result = handler
                        .expect("Expected command handler to be present")
                        .handle(command)
                        .await;

                    Self::send_command(&mut stream, result.unwrap()).await?;
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
        let msg = serde_json::to_vec(&command).expect("Couldn't serialize message");
        debug!("Sending command: {command:?}");
        stream.write(&msg).await
    }

    pub async fn start(self) -> Result<(), tokio::io::Error>
    where
        S: 'static,
    {
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

pub struct ArkeServerConfig<S: Clone> {
    pub bind_port: u16,
    pub bind_addr: IpAddr,
    pub certs: Vec<rustls::Certificate>,
    pub private_key: rustls::PrivateKey,
    handlers: HashMap<u32, Box<dyn ConversationHandler<S>>>,
}

impl<S: Clone> ArkeServerConfig<S> {
    pub fn builder() -> ArkeServerConfigBuilder<S> {
        ArkeServerConfigBuilder {
            bind_port: 8080,
            bind_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            certs: Vec::new(),
            private_key: None,
            handlers: None,
        }
    }
}

use std::collections::HashMap;
pub struct ArkeServerConfigBuilder<S: Clone> {
    bind_port: u16,
    bind_addr: IpAddr,
    certs: Vec<rustls::Certificate>,
    private_key: Option<rustls::PrivateKey>,
    handlers: Option<HashMap<u32, Box<dyn ConversationHandler<S>>>>,
}

impl<S: Clone> ArkeServerConfigBuilder<S> {
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

    pub fn build(self) -> ArkeServerConfig<S> {
        ArkeServerConfig {
            bind_port: self.bind_port,
            private_key: self.private_key.unwrap(),
            certs: self.certs,
            bind_addr: self.bind_addr,
            handlers: self.handlers.unwrap(),
        }
    }

    pub fn handlers(mut self, handlers: HashMap<u32, Box<dyn ConversationHandler<S>>>) -> Self {
        self.handlers = Some(handlers);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "type", content = "payload")]
#[repr(u32)]
pub enum ArkeCommand {
    Hello(String) = 0,
    CreateUser,
    Goodbye(Option<CommandError>),
}

impl ArkeCommand {
    pub fn discriminant(&self) -> u32 {
        unsafe { *<*const _>::from(self).cast::<u32>() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CommandError {
    ServerError { msg: String },
}

#[async_trait]
pub trait ConversationHandler<S: Clone>: Send {
    async fn handle(&mut self, command: ArkeCommand) -> Result<ArkeCommand, CommandError>;
}
