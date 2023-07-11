use log::{error, info, warn};
use rustls::{Certificate, PrivateKey, ServerConfig, ServerConnection};
use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    time::SystemTime,
};

mod user;

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use log::debug;

    use super::*;

    #[test]
    fn test_tls() {
        // Start a server
        std::thread::spawn(main);

        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));

        let mut reader = BufReader::new(std::fs::File::open("cert.pem").unwrap());
        let certs = rustls_pemfile::certs(&mut reader).unwrap();
        root_store.add_parsable_certificates(&certs);

        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let rc_config = Arc::new(config);
        let host = "localhost".try_into().unwrap();

        let mut client = rustls::ClientConnection::new(rc_config, host).unwrap();
        let mut socket = TcpStream::connect("localhost:8080").unwrap();

        'c: loop {
            if client.wants_read() {
                client.read_tls(&mut socket).unwrap();
                let state = client.process_new_packets().unwrap();

                if state.peer_has_closed() {
                    break 'c;
                }

                if state.plaintext_bytes_to_read() > 0 {
                    debug!(
                        "We have {} plaintext bytes to read",
                        state.plaintext_bytes_to_read()
                    );

                    let mut buf = vec![0; state.plaintext_bytes_to_read()].into_boxed_slice();
                    match client.reader().read(&mut buf) {
                        Ok(n) => {
                            debug!("Read {n} bytes!");
                            debug!("Client: {}", String::from_utf8(Vec::from(buf)).unwrap());
                            client.send_close_notify();
                        }
                        Err(err) => error!("{err}"),
                    }
                }
            }

            if client.wants_write() {
                client.write_tls(&mut socket).unwrap();
            }
        }
    }
}

fn process_socket(mut socket: TcpStream, tls_cfg: Arc<ServerConfig>) -> Result<(), std::io::Error> {
    info!("Connection opened from {}", socket.peer_addr()?);

    match ServerConnection::new(tls_cfg) {
        Ok(mut client) => 's: loop {
            if client.wants_write() {
                client.write_tls(&mut socket)?;
            }

            if client.wants_read() {
                client.read_tls(&mut socket)?;
                let state = client.process_new_packets().expect("TLS Error");

                if state.peer_has_closed() {
                    break 's;
                }

                if state.plaintext_bytes_to_read() > 0 {
                    let mut buffer = vec![0; state.plaintext_bytes_to_read()].into_boxed_slice();
                    client.reader().read_exact(&mut buffer).unwrap();

                    info!(
                        "Got message: {}",
                        String::from_utf8(Vec::from(buffer.clone())).unwrap().trim()
                    );

                    info!(
                        "Sending message: {}",
                        String::from_utf8(Vec::from(buffer.clone())).unwrap().trim()
                    );
                    client.writer().write_all(&buffer).unwrap();
                }
            }
        },
        Err(err) => error!("{err:?}"),
    }

    info!("Connection to {} closed", socket.peer_addr()?);
    Ok(())
}

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

fn main() -> Result<(), std::io::Error> {
    setup_logger().expect("Couldn't setup logger");

    if let Err(err) = dotenvy::dotenv() {
        warn!("Couldn't load .env file: {err:?}");
    }

    let bind_addr = env::var("BIND_ADDRESS").unwrap_or(String::from("127.0.0.1"));
    let bind_port = env::var("BIND_PORT").unwrap_or(String::from("8080"));

    info!("Starting TCP Listener on {bind_addr}:{bind_port}");
    let listener = TcpListener::bind(format!("{bind_addr}:{bind_port}"))?;

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

    let tls_config: Arc<ServerConfig> = Arc::new(
        ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, private_key.first().expect("No private key").clone())
            .expect("Couldn't parse certificates"),
    );

    loop {
        let cfg = Arc::clone(&tls_config);
        let (socket, _) = listener.accept()?;
        std::thread::spawn(move || process_socket(socket, cfg));
    }
}
