use std::path::Path;

use santi_core::{
    error::Result, model::runtime::SoulSession, port::soul_runtime::AcquireSoulSession,
};
use sqlx::SqlitePool;

mod bootstrap;
mod compact;
mod fork;
mod helpers;
mod query_port;
mod runtime_port;

#[derive(Clone)]
pub struct StandaloneSoulRuntime {
    pool: SqlitePool,
}

impl StandaloneSoulRuntime {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let pool = bootstrap::create_pool(path.as_ref()).await?;
        Ok(Self { pool })
    }

    async fn ensure_acquired_soul_session(
        &self,
        input: &AcquireSoulSession,
    ) -> Result<SoulSession> {
        self.ensure_soul_session(&input.soul_id, &input.session_id)
            .await?;
        self.fetch_session_soul(&input.session_id).await?.ok_or(
            santi_core::error::Error::NotFound {
                resource: "standalone_soul_session",
            },
        )
    }
}
