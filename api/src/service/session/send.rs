use std::sync::Arc;

use async_stream::try_stream;
use futures::Stream;
use uuid::Uuid;

use crate::{
    repo::{
        message_repo::{MessageRepo, NewMessage},
        relation_repo::RelationRepo,
        session_repo::SessionRepo,
        soul_repo::SoulRepo,
    },
    runtime::tools::ToolExecutor,
    service::{
        openai_compatible::OpenAiCompatibleProvider,
        session::kernel::{
            runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
            transcript,
        },
    },
};

#[derive(Clone)]
pub struct SessionSendService {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
    message_repo: Arc<MessageRepo>,
    relation_repo: Arc<RelationRepo>,
    provider: Arc<OpenAiCompatibleProvider>,
    tools: Arc<ToolExecutor>,
}

pub struct SendSessionCommand {
    pub session_id: String,
    pub user_content: String,
}

pub enum SendSessionEvent {
    OutputTextDelta(String),
    Completed,
}

impl SessionSendService {
    pub fn new(
        session_repo: Arc<SessionRepo>,
        soul_repo: Arc<SoulRepo>,
        message_repo: Arc<MessageRepo>,
        relation_repo: Arc<RelationRepo>,
        provider: Arc<OpenAiCompatibleProvider>,
        tools: Arc<ToolExecutor>,
    ) -> Self {
        Self {
            session_repo,
            soul_repo,
            message_repo,
            relation_repo,
            provider,
            tools,
        }
    }

    pub fn run(
        &self,
        cmd: SendSessionCommand,
    ) -> impl Stream<Item = Result<SendSessionEvent, String>> {
        let session_repo = self.session_repo.clone();
        let soul_repo = self.soul_repo.clone();
        let message_repo = self.message_repo.clone();
        let relation_repo = self.relation_repo.clone();
        let provider = self.provider.clone();
        let tools = self.tools.clone();

        try_stream! {
            let _ = provider;
            let _ = tools;

            if !session_repo
                .exists(&cmd.session_id)
                .await
                .map_err(|err| format!("session exists query failed: {err}"))?
            {
                Err("session not found".to_string())?;
            }

            let session = session_repo
                .get(&cmd.session_id)
                .await
                .map_err(|err| format!("session load failed: {err}"))?
                .ok_or_else(|| "session not found".to_string())?;

            let soul = soul_repo
                .get(&session.soul_id)
                .await
                .map_err(|err| format!("soul load failed: {err}"))?
                .ok_or_else(|| "soul not found".to_string())?;

            let mut tx = session_repo
                .begin_tx()
                .await
                .map_err(|err| format!("transaction begin failed: {err}"))?;

            let session_seq = session_repo
                .allocate_next_session_seq(&mut tx, &cmd.session_id)
                .await
                .map_err(|err| format!("session seq allocation failed: {err}"))?;

            let message_id = format!("msg_{}", Uuid::new_v4().simple());
            let message = message_repo
                .insert(
                    &mut tx,
                    NewMessage {
                        id: &message_id,
                        r#type: "user",
                        role: Some("user"),
                        content: &cmd.user_content,
                    },
                )
                .await
                .map_err(|err| format!("message insert failed: {err}"))?;

            relation_repo
                .attach_message_to_session(&mut tx, &cmd.session_id, &message.id, session_seq)
                .await
                .map_err(|err| format!("message relation insert failed: {err}"))?;

            tx.commit()
                .await
                .map_err(|err| format!("transaction commit failed: {err}"))?;

            let history = message_repo
                .list_for_session(&cmd.session_id)
                .await
                .map_err(|err| format!("message history load failed: {err}"))?;

            let runtime_context = tools.build_context(&cmd.session_id, &session.soul_id);
            let prompt = build_runtime_prompt(RuntimePromptSource {
                session_id: Some(cmd.session_id.clone()),
                soul_id: Some(session.soul_id.clone()),
                soul_memory: Some(soul.memory.clone()),
                session_memory: Some(session.memory.clone()),
                request_instructions: None,
                santi_runtime_soul_dir: Some(runtime_context.soul_dir.display().to_string()),
                santi_runtime_session_dir: Some(runtime_context.session_dir.display().to_string()),
                fallback_cwd: Some(runtime_context.fallback_cwd.display().to_string()),
            });
            let provider_input = history
                .iter()
                .filter_map(transcript::to_input_message)
                .collect::<Vec<_>>();

            let _instructions = prompt.render();
            let _provider_input = provider_input;

            yield SendSessionEvent::Completed;
        }
    }
}
