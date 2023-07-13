use log::warn;
use server::{ArkeServer, ArkeServerConfig};
use std::{env, net::Ipv4Addr, str::FromStr, time::SystemTime};
use tokio_rustls::rustls::{Certificate, PrivateKey};

mod server;
#[cfg(test)]
mod tests;
mod user;

#[cfg(debug_assertions)]
const LOG_LEVEL: log::LevelFilter = log::LevelFilter::Debug;
#[cfg(not(debug_assertions))]
const LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(LOG_LEVEL)
        .chain(std::io::stdout())
        .chain(fern::log_file(format!(
            "/var/log/alan/alan_log_{}.log",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ))?)
        .apply()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    setup_logger().expect("Couldn't setup logger");

    if let Err(err) = dotenvy::dotenv() {
        warn!("Couldn't load .env file: {err:?}");
    }

    let bind_addr = env::var("BIND_ADDRESS").unwrap_or(String::from("127.0.0.1"));
    let bind_port = env::var("BIND_PORT").unwrap_or(String::from("8080"));

    let mut reader = std::io::BufReader::new(
        std::fs::File::open("cert.pem").expect("Couldn't open certificate file"),
    );

    let certs = rustls_pemfile::certs(&mut reader)
        .expect("Couldn't read certificates")
        .into_iter()
        .map(Certificate)
        .collect::<Vec<_>>();

    let mut reader = std::io::BufReader::new(
        std::fs::File::open("privateKey.pem").expect("Couldn't open private key file"),
    );

    let private_key = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .expect("Couldn't read private key!")
        .into_iter()
        .map(PrivateKey)
        .collect::<Vec<_>>();

    let config = ArkeServerConfig::builder()
        .with_bind_addr(std::net::IpAddr::V4(
            Ipv4Addr::from_str(&bind_addr).expect("Invalid bind address"),
        ))
        .with_bind_port(u16::from_str(&bind_port).expect("Invalid bind port"))
        .with_certs(certs)
        .with_private_key(private_key.first().unwrap().clone())
        .build();

    let server = ArkeServer::new(config)
        .await
        .expect("Couldn't create server");
    server.start().await
}
