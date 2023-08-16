use arke::{server::{command::{ArkeHello, ArkeCommand}, ArkeServer, db::Entity}, user::User};
use log::warn;
use arke::server::{state::State, command::CommandError};
use macros::command_handler;
use openssl::ec::EcKey;
use std::{env, net::Ipv4Addr, str::FromStr, time::SystemTime, sync::Arc};
use tokio_rustls::rustls::{Certificate, PrivateKey};
use tokio::sync::Mutex;

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
        .apply()?;
    Ok(())
}

#[command_handler(
    state = "state",
    command(
        ArkeCommand::Hello(ArkeHello { version: (major, minor, _patch) }), 
        CommandError::ServerError { 
            msg: "Invalid command".to_string() 
        }.into()
    )
)]
async fn hello(state: State, command: ArkeCommand) -> ArkeCommand {
    let hello = ArkeHello::default();
    let (server_major, server_minor, _) = hello.version; 
    
    if server_major != major || server_minor != minor {
        CommandError::ServerError { msg: "Server and client have a version mismatch!".to_string() }.into()
    } else {
        state.handshake = true;
        ArkeCommand::Hello(hello)
    }
}

#[command_handler(state = "state", command(
    ArkeCommand::CreateUser(new_user), 
    CommandError::ServerError {
        msg: "Invalid command".to_string()
    }.into()
))]
async fn create_user(state: State, command: ArkeCommand) -> ArkeCommand {
    let identity_key = if let Ok(key) = new_user.identity_key.ec_key() {
        key
    } else {
        return ArkeCommand::Error(CommandError::InvalidKey);
    };
    
    if let Ok(sig) = openssl::ecdsa::EcdsaSig::from_der(&new_user.prekey_signature) {
        if let Ok(true) = sig.verify(new_user.signed_prekey.as_ref(), identity_key.as_ref()) {
        } else {
            return ArkeCommand::Error(CommandError::InvalidSignature { msg: "Prekey signature is invalid".to_string() }).into();
        }
    } else {
        return ArkeCommand::Error(CommandError::InvalidSignature { msg: "Prekey signature is invalid".to_string() }).into();
    }
    
    if let Err(err) = User::from(new_user).insert(&state.db).await {
        log::error!("Couldn't create new user: {err:?}");
        CommandError::ServerError {
            msg: "Couldn't create new user!".to_string()
        }.into()
    } else {
        ArkeCommand::Success
    }
}

#[command_handler(
    state = "_state",
    command(
        ArkeCommand::Goodbye(_),
        CommandError::ServerError {
            msg: "Invalid command".to_string()
        }.into()
    )
)]
async fn goodbye(_state: State, command: ArkeCommand) -> ArkeCommand {
    ArkeCommand::Goodbye(None)
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
        .collect::<Vec<_>>()
        .first()
        .expect("No private key")
        .clone();

    let pool = sqlx::mysql::MySqlPool::connect(env::var("DATABASE_URL").unwrap().as_ref()).await.unwrap();

    let state = Arc::new(Mutex::new(State::new("localhost", pool)));
    let server = ArkeServer::builder()
        .with_bind_addr(std::net::IpAddr::V4(
            Ipv4Addr::from_str(&bind_addr).expect("Invalid bind address"),
        ))
        .with_bind_port(u16::from_str(&bind_port).expect("Invalid bind port"))
        .with_certs(certs)
        .with_private_key(private_key)
        .handlers(arke::routes! {
            Arc::clone(&state),
            ArkeCommand::Hello => hello,
            ArkeCommand::CreateUser => create_user,
            ArkeCommand::Goodbye => goodbye
        })
        .build()
        .await
        .expect("Couldn't build server!");

    server.start().await
}
