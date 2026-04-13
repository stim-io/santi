use santi_core::{
    error::{Error, Result},
    model::runtime::{AssemblyItem, AssemblyTarget, Compact, SoulSessionEntry, SoulSessionTargetType},
    port::{
        compact_ledger::CompactLedgerPort,
        compact_runtime::{AppendCompact, CompactRuntimePort},
    },
};
use sqlx::Row;

use super::{helpers::map_compact_row, StandaloneSoulRuntime};

#[async_trait::async_trait]
impl CompactLedgerPort for StandaloneSoulRuntime {
    async fn list_compacts(&self, soul_session_id: &str) -> Result<Vec<Compact>> {
        let session_id: String = sqlx::query_scalar(
            r#"SELECT session_id FROM standalone_soul_sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(soul_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load standalone compact session id failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        let rows = sqlx::query(
            r#"SELECT id, turn_id, summary, start_session_seq, end_session_seq, created_at
               FROM standalone_session_compacts
               WHERE session_id = ?1
               ORDER BY created_at ASC"#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("list standalone compacts failed: {err}"),
        })?;

        Ok(rows.into_iter().map(map_compact_row).collect())
    }
}

#[async_trait::async_trait]
impl CompactRuntimePort for StandaloneSoulRuntime {
    async fn append_compact(&self, input: AppendCompact) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone append compact tx begin failed: {err}"),
        })?;

        let turn_row = sqlx::query(
            r#"SELECT t.soul_session_id, ss.session_id
               FROM standalone_turns t
               JOIN standalone_soul_sessions ss ON ss.id = t.soul_session_id
               WHERE t.id = ?1
               LIMIT 1"#,
        )
        .bind(&input.turn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load standalone compact turn failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_turn",
        })?;

        let soul_session_id: String = turn_row.get("soul_session_id");
        let session_id: String = turn_row.get("session_id");

        let seq_row = sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append compact seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let compact_row = sqlx::query(
            r#"INSERT INTO standalone_session_compacts
               (id, session_id, turn_id, summary, start_session_seq, end_session_seq)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               RETURNING id, turn_id, summary, start_session_seq, end_session_seq, created_at"#,
        )
        .bind(&input.compact_id)
        .bind(&session_id)
        .bind(&input.turn_id)
        .bind(&input.summary)
        .bind(input.start_session_seq)
        .bind(input.end_session_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert standalone compact failed: {err}"),
        })?;

        let entry_row = sqlx::query(
            r#"INSERT INTO standalone_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'compact', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&soul_session_id)
        .bind(&input.compact_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert standalone compact assembly entry failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone append compact tx commit failed: {err}"),
        })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::Compact,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: entry_row.get("created_at"),
            },
            target: AssemblyTarget::Compact(map_compact_row(compact_row)),
        })
    }
}
