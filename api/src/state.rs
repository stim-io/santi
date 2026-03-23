use std::sync::Arc;

use crate::{
    db,
    repo::{
        message_repo::MessageRepo,
        relation_repo::RelationRepo,
        session_repo::SessionRepo,
        soul_repo::SoulRepo,
    },
    runtime::tools::ToolExecutor,
    service::{
        openai_compatible::OpenAiCompatibleProvider,
        session::{memory::SessionMemoryService, query::SessionQueryService, send::SessionSendService},
    },
};

#[derive(Clone)]
pub struct AppState {
    session_memory: Arc<SessionMemoryService>,
    session_query: Arc<SessionQueryService>,
    session_send: Arc<SessionSendService>,
}

impl AppState {
    pub async fn new(config: crate::config::Config) -> Self {
        let provider = OpenAiCompatibleProvider::new(
            config.openai_api_key.clone(),
            config.openai_base_url.clone(),
        );
        let pool = db::init_postgres(&config.database_url)
            .await
            .expect("postgres init failed");
        let session_repo = Arc::new(SessionRepo::new(pool.clone()));
        let soul_repo = Arc::new(SoulRepo::new(pool.clone()));
        let message_repo = Arc::new(MessageRepo::new(pool.clone()));
        let relation_repo = Arc::new(RelationRepo::new());
        let provider = Arc::new(provider);
        let session_memory = Arc::new(SessionMemoryService::new(session_repo.clone(), soul_repo.clone()));
        let session_query = Arc::new(SessionQueryService::new(
            session_repo.clone(),
            soul_repo.clone(),
            message_repo.clone(),
        ));
        let tools = Arc::new(ToolExecutor::new(
            session_memory.as_ref().clone(),
            config.runtime_root.clone(),
            config.execution_root.clone(),
        ));
        let session_send = Arc::new(SessionSendService::new(
            session_repo.clone(),
            soul_repo.clone(),
            message_repo.clone(),
            relation_repo.clone(),
            provider.clone(),
            tools.clone(),
        ));

        Self {
            session_memory,
            session_query,
            session_send,
        }
    }

    pub fn session_send(&self) -> Arc<SessionSendService> {
        self.session_send.clone()
    }

    pub fn session_memory(&self) -> Arc<SessionMemoryService> {
        self.session_memory.clone()
    }

    pub fn session_query(&self) -> Arc<SessionQueryService> {
        self.session_query.clone()
    }
}
