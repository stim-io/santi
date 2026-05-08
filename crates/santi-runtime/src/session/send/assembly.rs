use std::{collections::BTreeSet, sync::Arc};

use santi_core::{
    model::{
        runtime::{AssemblyItem, AssemblyTarget},
        session::SessionMessage,
    },
    port::session_ledger::SessionLedgerPort,
    provider::ProviderInputMessage,
    service::session::kernel::transcript,
};

use super::{map_core_error, SendSessionError};

pub(super) async fn build_assembly_items(
    session_ledger: Arc<dyn SessionLedgerPort>,
    session_id: &str,
    soul_session_id: &str,
) -> Result<Vec<AssemblyItem>, SendSessionError> {
    let session_messages = session_ledger
        .list_messages(session_id, None)
        .await
        .map_err(map_core_error)?;
    let mut items = Vec::new();
    for message in session_messages {
        let Some(message) = session_ledger
            .get_message(&message.message.id)
            .await
            .map_err(map_core_error)?
        else {
            continue;
        };
        items.push(AssemblyItem {
            entry: santi_core::model::runtime::SoulSessionEntry {
                soul_session_id: soul_session_id.to_string(),
                target_type: santi_core::model::runtime::SoulSessionTargetType::Message,
                target_id: message.message.id.clone(),
                soul_session_seq: message.relation.session_seq,
                created_at: message.relation.created_at.clone(),
            },
            target: AssemblyTarget::Message(message),
        });
    }
    Ok(items)
}

fn item_to_input_message(item: &AssemblyItem) -> Option<ProviderInputMessage> {
    match &item.target {
        AssemblyTarget::Message(message) => transcript::to_input_message(message),
        AssemblyTarget::Compact(compact) => transcript::compact_to_input_message(compact),
        AssemblyTarget::ToolCall(_) | AssemblyTarget::ToolResult(_) => None,
    }
}

pub fn assembly_to_provider_input(items: &[AssemblyItem]) -> Vec<ProviderInputMessage> {
    let effective_compact_indexes = effective_compact_indexes(items);

    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match &item.target {
            AssemblyTarget::Message(message) => {
                if message_is_compacted(
                    message.relation.session_seq,
                    items,
                    &effective_compact_indexes,
                ) {
                    None
                } else {
                    transcript::to_input_message(message)
                }
            }
            AssemblyTarget::Compact(_) if effective_compact_indexes.contains(&index) => {
                item_to_input_message(item)
            }
            AssemblyTarget::Compact(_)
            | AssemblyTarget::ToolCall(_)
            | AssemblyTarget::ToolResult(_) => None,
        })
        .collect()
}

fn effective_compact_indexes(items: &[AssemblyItem]) -> BTreeSet<usize> {
    let compact_ranges = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match &item.target {
            AssemblyTarget::Compact(compact) => {
                Some((index, compact.start_session_seq, compact.end_session_seq))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    compact_ranges
        .iter()
        .filter(|(index, start, end)| {
            !compact_ranges
                .iter()
                .any(|(other_index, other_start, other_end)| {
                    other_index > index && other_start <= start && other_end >= end
                })
        })
        .map(|(index, _, _)| *index)
        .collect()
}

fn message_is_compacted(
    session_seq: i64,
    items: &[AssemblyItem],
    effective_compact_indexes: &BTreeSet<usize>,
) -> bool {
    items.iter().enumerate().any(|(index, item)| {
        if !effective_compact_indexes.contains(&index) {
            return false;
        }

        match &item.target {
            AssemblyTarget::Compact(compact) => {
                compact.start_session_seq <= session_seq && session_seq <= compact.end_session_seq
            }
            _ => false,
        }
    })
}

#[allow(dead_code)]
fn _message_id(message: &SessionMessage) -> &str {
    &message.message.id
}
