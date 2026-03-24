use sqlx::PgPool;

pub async fn seed_default_soul(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO souls (id, memory)
        VALUES ($1, $2)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind("soul_default")
    .bind("")
    .execute(pool)
    .await?;

    Ok(())
}
