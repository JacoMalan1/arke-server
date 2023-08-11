use async_trait::async_trait;
use sqlx::mysql::{MySqlPool, MySqlQueryResult};

#[async_trait]
pub trait Entity {
    async fn insert(&self, db: &MySqlPool) -> Result<MySqlQueryResult, sqlx::Error>;
}
