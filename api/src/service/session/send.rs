use std::sync::Arc;

use async_stream::try_stream;
use futures::{Stream, StreamExt};
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
        openai_compatible::{OpenAiCompatibleProvider, ProviderEvent},
        session::kernel::{
            runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
            transcript,
        },
        turn::ProviderRequest,
    },
};

#[derive(Clone)]
pub struct SessionSendService {
    model: String,
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
        model: String,
        session_repo: Arc<SessionRepo>,
        soul_repo: Arc<SoulRepo>,
        message_repo: Arc<MessageRepo>,
        relation_repo: Arc<RelationRepo>,
        provider: Arc<OpenAiCompatibleProvider>,
        tools: Arc<ToolExecutor>,
    ) -> Self {
        Self {
            model,
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
        let model = self.model.clone();
        let session_repo = self.session_repo.clone();
        let soul_repo = self.soul_repo.clone();
        let message_repo = self.message_repo.clone();
        let relation_repo = self.relation_repo.clone();
        let provider = self.provider.clone();
        let tools = self.tools.clone();

        try_stream! {
            tracing::info!(session_id = %cmd.session_id, "session send started");

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

            tracing::info!(session_id = %cmd.session_id, message_id = %message.id, session_seq, "user message persisted");

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

            let instructions = prompt.render();

            tracing::info!(session_id = %cmd.session_id, input_messages = provider_input.len(), model = %model, "provider request dispatched");

            let mut assistant_text = String::new();
            let mut saw_completed = false;
            let stream = provider.stream_response(ProviderRequest {
                model,
                instructions,
                input: provider_input,
                tools: None,
                previous_response_id: None,
                function_call_output: None,
            });
            futures::pin_mut!(stream);

            while let Some(event) = stream.next().await {
                match event.map_err(|err| format!("provider stream failed: {err}"))? {
                    ProviderEvent::OutputTextDelta(delta) => {
                        assistant_text.push_str(&delta);
                        yield SendSessionEvent::OutputTextDelta(delta);
                    }
                    ProviderEvent::Completed { .. } => {
                        saw_completed = true;
                        break;
                    }
                    _ => {}
                }
            }

            if !saw_completed {
                Err("provider stream ended before completion".to_string())?;
            }

            let mut tx = session_repo
                .begin_tx()
                .await
                .map_err(|err| format!("assistant transaction begin failed: {err}"))?;

            let assistant_seq = session_repo
                .allocate_next_session_seq(&mut tx, &cmd.session_id)
                .await
                .map_err(|err| format!("assistant session seq allocation failed: {err}"))?;

            let assistant_message_id = format!("msg_{}", Uuid::new_v4().simple());
            let assistant_message = message_repo
                .insert(
                    &mut tx,
                    NewMessage {
                        id: &assistant_message_id,
                        r#type: "assistant",
                        role: Some("assistant"),
                        content: &assistant_text,
                    },
                )
                .await
                .map_err(|err| format!("assistant message insert failed: {err}"))?;

            relation_repo
                .attach_message_to_session(&mut tx, &cmd.session_id, &assistant_message.id, assistant_seq)
                .await
                .map_err(|err| format!("assistant relation insert failed: {err}"))?;

            tx.commit()
                .await
                .map_err(|err| format!("assistant transaction commit failed: {err}"))?;

            tracing::info!(session_id = %cmd.session_id, message_id = %assistant_message.id, session_seq = assistant_seq, output_chars = assistant_text.len(), "session send completed");

            yield SendSessionEvent::Completed;
        }
    }
}
