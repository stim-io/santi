use sqlx::Row;

use santi_core::{
    error::{Error, Result},
    model::runtime::{
        AssemblyItem, AssemblyTarget, Compact, SoulSessionEntry, SoulSessionTargetType,
    },
    port::{
        compact_ledger::CompactLedgerPort,
        compact_runtime::{AppendCompact, CompactRuntimePort},
    },
};

use super::{helpers::map_compact_row, DbSoulRuntime};

#[async_trait::async_trait]
impl CompactRuntimePort for DbSoulRuntime {
    async fn append_compact(&self, input: AppendCompact) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("append compact tx begin failed: {err}"),
        })?;

        sqlx::query(
            r#"
            INSERT INTO compacts (id, turn_id, summary, start_session_seq, end_session_seq)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&input.compact_id)
        .bind(&input.turn_id)
        .bind(&input.summary)
        .bind(input.start_session_seq)
        .bind(input.end_session_seq)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert compact failed: {err}"),
        })?;

        let soul_session_id: String =
            sqlx::query_scalar(r#"SELECT soul_session_id FROM turns WHERE id = $1 LIMIT 1"#)
                .bind(&input.turn_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("load compact soul session failed: {err}"),
                })?
                .ok_or(Error::NotFound { resource: "turn" })?;

        let allocated_seq = Self::allocate_seq(&mut tx, &soul_session_id).await?;

        let entry_row = sqlx::query(
            r#"
            INSERT INTO r_soul_session_messages (soul_session_id, target_type, target_id, soul_session_seq)
            VALUES ($1, 'compact', $2, $3)
            RETURNING soul_session_id, target_id, soul_session_seq,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            "#,
        )
        .bind(&soul_session_id)
        .bind(&input.compact_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert compact assembly entry failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("append compact tx commit failed: {err}"),
        })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::Compact,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: entry_row.get("created_at"),
            },
            target: AssemblyTarget::Compact(Compact {
                id: input.compact_id,
                turn_id: input.turn_id,
                summary: input.summary,
                start_session_seq: input.start_session_seq,
                end_session_seq: input.end_session_seq,
                created_at: entry_row.get("created_at"),
            }),
        })
    }
}

#[async_trait::async_trait]
impl CompactLedgerPort for DbSoulRuntime {
    async fn list_compacts(&self, soul_session_id: &str) -> Result<Vec<Compact>> {
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.turn_id, c.summary, c.start_session_seq, c.end_session_seq,
                   to_char(c.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            FROM compacts c
            JOIN turns t ON t.id = c.turn_id
            WHERE t.soul_session_id = $1
            ORDER BY c.created_at ASC, c.id ASC
            "#,
        )
        .bind(soul_session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("list compacts failed: {err}"),
        })?;

        Ok(rows.into_iter().map(map_compact_row).collect())
    }
}
