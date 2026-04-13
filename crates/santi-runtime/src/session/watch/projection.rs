use santi_core::{
    model::{effect::SessionEffect, message::MessagePart, session::SessionMessage},
    port::effect_ledger::EffectLedgerPort,
};

use crate::session::query::SessionQueryService;

use super::shapes::{
    SessionWatchEffectSummary, SessionWatchMessageSummary, SessionWatchSnapshot,
};

pub(super) async fn get_session_watch_snapshot(
    query: &SessionQueryService,
    effect_ledger: &dyn EffectLedgerPort,
    session_id: &str,
) -> Result<Option<SessionWatchSnapshot>, String> {
    let Some(session) = query.get_session(session_id).await? else {
        return Ok(None);
    };

    let messages = query.list_session_messages(session_id).await?;
    let effects = effect_ledger
        .list_effects(session_id)
        .await
        .map_err(|err| format!("{err:?}"))?;

    let latest_seq = messages
        .last()
        .map(|message| message.relation.session_seq)
        .unwrap_or(0);

    Ok(Some(SessionWatchSnapshot {
        session_id: session.id,
        latest_seq,
        messages: messages
            .into_iter()
            .map(SessionWatchMessageSummary::from)
            .collect(),
        effects: effects
            .into_iter()
            .map(SessionWatchEffectSummary::from)
            .collect(),
    }))
}

impl From<SessionMessage> for SessionWatchMessageSummary {
    fn from(value: SessionMessage) -> Self {
        Self {
            id: value.message.id,
            actor_type: format!("{:?}", value.message.actor_type).to_lowercase(),
            actor_id: value.message.actor_id,
            session_seq: value.relation.session_seq,
            content_text: value
                .message
                .content
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    MessagePart::Image { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            state: format!("{:?}", value.message.state).to_lowercase(),
            created_at: value.message.created_at,
        }
    }
}

impl From<SessionEffect> for SessionWatchEffectSummary {
    fn from(value: SessionEffect) -> Self {
        Self {
            id: value.id,
            effect_type: value.effect_type,
            status: value.status,
            source_hook_id: value.source_hook_id,
            result_ref: value.result_ref,
            error_text: value.error_text,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
