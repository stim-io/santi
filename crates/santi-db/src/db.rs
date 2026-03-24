pub mod migrate;
pub mod pool;
pub mod seed;

pub use sqlx::{PgPool, Postgres, Transaction};

pub type DbResult<T> = Result<T, sqlx::Error>;

pub async fn init_postgres(database_url: &str) -> DbResult<PgPool> {
    let pool = pool::connect(database_url).await?;
    migrate::run(&pool).await?;
    seed::seed_default_soul(&pool).await?;
    Ok(pool)
}
