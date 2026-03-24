use std::sync::Arc;

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use santi_core::{
    port::{
        provider::{Provider, ProviderEvent, ProviderRequest},
        turn_store::{NewTurnMessage, TurnStore},
    },
    provider::ProviderInputMessage,
    service::session::kernel::{
        runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
        transcript,
    },
};
use santi_db::adapter::turn_store::RepoBackedTurnStore;
use santi_lock::{RedisLockClient, RedisLockError};
use santi_provider::openai_compatible::OpenAiCompatibleProvider;
use tokio::sync::mpsc;

use crate::runtime::tools::ToolExecutor;

#[derive(Clone)]
pub struct SessionSendService {
    model: String,
    lock_client: Arc<RedisLockClient>,
    turn_store: Arc<RepoBackedTurnStore>,
    provider: Arc<OpenAiCompatibleProvider>,
    tools: Arc<ToolExecutor>,
}

pub struct SendSessionCommand {
    pub session_id: String,
    pub user_content: String,
}

#[derive(Clone, Debug)]
pub enum SendSessionError {
    Busy,
    NotFound,
    Internal(String),
}

pub enum SendSessionEvent {
    Started,
    OutputTextDelta(String),
    Completed,
}

impl SessionSendService {
    pub fn new(
        model: String,
        lock_client: Arc<RedisLockClient>,
        turn_store: Arc<RepoBackedTurnStore>,
        provider: Arc<OpenAiCompatibleProvider>,
        tools: Arc<ToolExecutor>,
    ) -> Self {
        Self {
            model,
            lock_client,
            turn_store,
            provider,
            tools,
        }
    }

    pub fn run(
        &self,
        cmd: SendSessionCommand,
    ) -> impl Stream<Item = Result<SendSessionEvent, SendSessionError>> {
        let model = self.model.clone();
        let lock_client = self.lock_client.clone();
        let turn_store = self.turn_store.clone();
        let provider = self.provider.clone();
        let tools = self.tools.clone();

        try_stream! {
            let session_id = cmd.session_id.clone();
            let (tx, mut rx) = mpsc::unbounded_channel::<Result<SendSessionEvent, SendSessionError>>();
            let error_tx = tx.clone();

            tokio::spawn(async move {
                let tx = tx.clone();
                let result = lock_client
                    .with_lock(format!("lock:session_send:{}", session_id), |_lock| async move {
                        tracing::info!(session_id = %cmd.session_id, "session send started");

                        let turn_context = turn_store
                            .load_turn_context(&cmd.session_id)
                            .await
                            .map_err(|err| RedisLockError::Redis { message: format!("turn context load failed: {err:?}") })?
                            .ok_or_else(|| RedisLockError::Redis { message: "session not found".to_string() })?;

                        let _ = tx.send(Ok(SendSessionEvent::Started));

                        let message = turn_store
                            .append_message(
                                &cmd.session_id,
                                NewTurnMessage {
                                    r#type: "user".to_string(),
                                    role: Some("user".to_string()),
                                    content: cmd.user_content.clone(),
                                },
                            )
                            .await
                            .map_err(|err| RedisLockError::Redis { message: format!("user message append failed: {err:?}") })?;

                        tracing::info!(session_id = %cmd.session_id, message_id = %message.id, "user message persisted");

                        let history = turn_store
                            .list_messages(&cmd.session_id)
                            .await
                            .map_err(|err| RedisLockError::Redis { message: format!("message history load failed: {err:?}") })?;

                        let runtime_context = tools.build_context(&cmd.session_id, &turn_context.session.soul_id);
                        let prompt = build_runtime_prompt(RuntimePromptSource {
                            session_id: Some(cmd.session_id.clone()),
                            soul_id: Some(turn_context.session.soul_id.clone()),
                            soul_memory: Some(turn_context.soul_memory.clone()),
                            session_memory: Some(turn_context.session.memory.clone()),
                            request_instructions: None,
                            santi_runtime_soul_dir: Some(runtime_context.soul_dir.display().to_string()),
                            santi_runtime_session_dir: Some(runtime_context.session_dir.display().to_string()),
                            fallback_cwd: Some(runtime_context.fallback_cwd.display().to_string()),
                        });
                        let provider_input: Vec<ProviderInputMessage> = history
                            .iter()
                            .filter_map(transcript::to_input_message)
                            .collect();

                        let instructions = prompt.render();

                        tracing::info!(session_id = %cmd.session_id, input_messages = provider_input.len(), model = %model, "provider request dispatched");

                        let mut assistant_text = String::new();
                        let mut saw_completed = false;
                        let stream = provider.stream(ProviderRequest {
                            model,
                            instructions,
                            input: provider_input,
                            tools: None,
                            previous_response_id: None,
                            function_call_output: None,
                        });
                        futures::pin_mut!(stream);

                        while let Some(event) = stream.next().await {
                            match event.map_err(|err| RedisLockError::Redis { message: format!("provider stream failed: {err:?}") })? {
                                ProviderEvent::OutputTextDelta(delta) => {
                                    assistant_text.push_str(&delta);
                                    let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(delta)));
                                }
                                ProviderEvent::Completed { .. } => {
                                    saw_completed = true;
                                    break;
                                }
                            }
                        }

                        if !saw_completed {
                            return Err(RedisLockError::Redis { message: "provider stream ended before completion".to_string() });
                        }

                        let assistant_message = turn_store
                            .append_message(
                                &cmd.session_id,
                                NewTurnMessage {
                                    r#type: "assistant".to_string(),
                                    role: Some("assistant".to_string()),
                                    content: assistant_text.clone(),
                                },
                            )
                            .await
                            .map_err(|err| RedisLockError::Redis { message: format!("assistant message append failed: {err:?}") })?;

                        tracing::info!(session_id = %cmd.session_id, message_id = %assistant_message.id, output_chars = assistant_text.len(), "session send completed");

                        let _ = tx.send(Ok(SendSessionEvent::Completed));
                        Ok(())
                    })
                    .await;

                if let Err(err) = result {
                    let error = match err {
                        RedisLockError::Busy { .. } => SendSessionError::Busy,
                        RedisLockError::Redis { message } if message == "session not found" => SendSessionError::NotFound,
                        RedisLockError::Lost { .. } => SendSessionError::Internal("session send lock lost".to_string()),
                        RedisLockError::Redis { message } => SendSessionError::Internal(message),
                        RedisLockError::Release { message, .. } => SendSessionError::Internal(message),
                        RedisLockError::InvalidConfig { message } => SendSessionError::Internal(message),
                    };
                    let _ = error_tx.send(Err(error));
                }
            });

            while let Some(event) = rx.recv().await {
                match event {
                    Ok(event) => yield event,
                    Err(err) => Err(err)?,
                }
            }
        }
    }
}
