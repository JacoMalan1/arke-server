use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{rustls, server::TlsStream, TlsAcceptor};

pub struct ArkeServer {
    listener: TcpListener,
    certs: Vec<rustls::Certificate>,
    private_key: rustls::PrivateKey,
}

impl ArkeServer {
    pub async fn new(config: ArkeServerConfig) -> Result<Self, std::io::Error> {
        info!(
            "Arke server will bind to {}:{}",
            config.bind_addr, config.bind_port
        );

        Ok(Self {
            listener: TcpListener::bind(format!("{}:{}", config.bind_addr, config.bind_port))
                .await?,
            certs: config.certs,
            private_key: config.private_key,
        })
    }

    async fn handle_connection(
        stream: TcpStream,
        acceptor: TlsAcceptor,
    ) -> Result<(), tokio::io::Error> {
        let peer_addr = stream.peer_addr()?;
        let mut stream = acceptor.accept(stream).await?;

        'connection: loop {
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).await?;

            debug!("Received message: {}", hex::encode(&buffer[..n]));

            match rmp_serde::from_slice::<ArkeCommand>(&buffer[..n]) {
                Ok(command) => {
                    debug!("Received command: {command:?}");
                    match command.ty {
                        ArkeCommandType::Goodbye => {
                            Self::send_command(
                                &mut stream,
                                ArkeCommand::new(ArkeCommandType::Goodbye, None),
                            )
                            .await?;
                            break 'connection;
                        }
                        ArkeCommandType::Hello => {
                            Self::send_command(
                                &mut stream,
                                ArkeCommand::new(
                                    ArkeCommandType::Hello,
                                    Some(format!(
                                        "Hello {}",
                                        command.payload.unwrap_or("".to_string())
                                    )),
                                ),
                            )
                            .await?;
                        }
                    }
                }
                Err(err) => {
                    error!("Invalid command. {err:?}");
                    Self::send_command(
                        &mut stream,
                        ArkeCommand::new(ArkeCommandType::Goodbye, None),
                    )
                    .await?;
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
        let msg = rmp_serde::to_vec(&command).expect("Couldn't serialize message");
        debug!("Sending message: {}", hex::encode(&msg));
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
        loop {
            let acceptor = acceptor.clone();
            let (socket, peer_addr) = self.listener.accept().await?;
            info!("Accepting socket connection from {peer_addr}");
            tokio::spawn(async move { Self::handle_connection(socket, acceptor).await });
        }
    }
}

pub struct ArkeServerConfig {
    pub bind_port: u16,
    pub bind_addr: IpAddr,
    pub certs: Vec<rustls::Certificate>,
    pub private_key: rustls::PrivateKey,
}

impl ArkeServerConfig {
    pub fn builder() -> ArkeServerConfigBuilder {
        ArkeServerConfigBuilder {
            bind_port: 8080,
            bind_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            certs: Vec::new(),
            private_key: None,
        }
    }
}

pub struct ArkeServerConfigBuilder {
    bind_port: u16,
    bind_addr: IpAddr,
    certs: Vec<rustls::Certificate>,
    private_key: Option<rustls::PrivateKey>,
}

impl ArkeServerConfigBuilder {
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

    pub fn build(self) -> ArkeServerConfig {
        ArkeServerConfig {
            bind_port: self.bind_port,
            private_key: self.private_key.unwrap(),
            certs: self.certs,
            bind_addr: self.bind_addr,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ArkeCommandType {
    Hello,
    Goodbye,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkeCommand {
    #[serde(rename = "type")]
    pub ty: ArkeCommandType,
    pub payload: Option<String>,
}

impl ArkeCommand {
    pub fn new(ty: ArkeCommandType, payload: Option<String>) -> Self {
        Self { ty, payload }
    }
}
