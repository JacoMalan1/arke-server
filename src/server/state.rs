use sqlx::mysql::MySqlPool;

#[derive(Debug)]
pub struct State {
    pub hostname: &'static str,
    pub handshake: bool,
    pub db: MySqlPool,
}

impl State {
    pub fn new(hostname: &'static str, db: MySqlPool) -> Self {
        Self {
            hostname,
            db,
            handshake: false,
        }
    }
}
