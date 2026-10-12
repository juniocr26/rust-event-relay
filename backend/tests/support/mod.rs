use sqlx::{
    Executor, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{future::Future, time::Duration};
use uuid::Uuid;

pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
const WAIT: Duration = Duration::from_secs(10);

// Keep raw driver errors out of assertion output; retain operation and SQLSTATE only.
pub fn checked<T>(result: Result<T, sqlx::Error>, operation: &str) -> T {
    result.unwrap_or_else(|error| {
        let code = error.as_database_error().and_then(|e| e.code());
        panic!("{operation} failed (SQLSTATE {code:?}); check PostgreSQL configuration/CREATE DATABASE privilege")
    })
}

pub async fn isolated<F, Fut>(case: F)
where
    F: FnOnce(PgPool) -> Fut + Send + 'static,
    Fut: Future<Output = TestResult> + Send + 'static,
{
    let required = |key| std::env::var(key).unwrap_or_else(|_| panic!("missing required {key}"));
    let options = PgConnectOptions::new()
        .host(&std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres".into()))
        .port(
            std::env::var("POSTGRES_PORT")
                .unwrap_or_else(|_| "5432".into())
                .parse()
                .expect("invalid POSTGRES_PORT"),
        )
        .username(&required("POSTGRES_USER"))
        .password(&required("POSTGRES_PASSWORD"))
        .database(&required("POSTGRES_DB"))
        .options([("statement_timeout", "5000"), ("lock_timeout", "3000")]);
    let make_pool = |options| {
        PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(WAIT)
            .connect_with(options)
    };
    let admin = checked(
        tokio::time::timeout(WAIT, make_pool(options.clone()))
            .await
            .expect("admin connection deadline"),
        "admin connection",
    );
    let name = format!("relay_it_{}", Uuid::now_v7().simple());
    println!("isolated database: {name}");
    // Identifier consists exclusively of a fixed prefix and hexadecimal UUID.
    checked(
        tokio::time::timeout(
            WAIT,
            admin.execute(format!("CREATE DATABASE \"{name}\"").as_str()),
        )
        .await
        .expect("create database deadline"),
        "create isolated database",
    );
    let task_pool = match tokio::time::timeout(WAIT, make_pool(options.database(&name))).await {
        Ok(Ok(pool)) => Some(pool),
        _ => None,
    };
    let outcome = if let Some(pool) = task_pool.as_ref() {
        let pool = pool.clone();
        let mut task = tokio::spawn(async move {
            for migration in [
                include_str!("../../migrations/20261007000000_create_relay_schema.up.sql"),
                include_str!("../../migrations/20261007175358_create_outbox_events.up.sql"),
            ] {
                checked(
                    sqlx::raw_sql(migration).execute(&pool).await,
                    "apply migration",
                );
            }
            case(pool).await
        });
        match tokio::time::timeout(Duration::from_secs(45), &mut task).await {
            Ok(result) => Some(result),
            Err(_) => {
                task.abort();
                let _ = tokio::time::timeout(WAIT, task).await;
                None
            }
        }
    } else {
        None
    };
    if let Some(pool) = task_pool {
        tokio::time::timeout(WAIT, pool.close())
            .await
            .expect("test pool close deadline");
    }
    checked(
        tokio::time::timeout(
            WAIT,
            admin.execute(format!("DROP DATABASE \"{name}\"").as_str()),
        )
        .await
        .expect("cleanup deadline"),
        "drop isolated database",
    );
    let remains: bool = checked(
        tokio::time::timeout(
            WAIT,
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname=$1)")
                .bind(&name)
                .fetch_one(&admin),
        )
        .await
        .expect("catalog verification deadline"),
        "verify cleanup",
    );
    assert!(!remains, "isolated database remains: {name}");
    tokio::time::timeout(WAIT, admin.close())
        .await
        .expect("admin pool close deadline");
    match outcome {
        Some(Ok(Ok(()))) => {}
        Some(Err(error)) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Some(Ok(Err(error))) => {
            if let Some(error) =
                error.downcast_ref::<reliable_event_relay::persistence::PersistenceError>()
            {
                panic!("integration case returned an error: {error} (source redacted)");
            }
            panic!("integration case returned an error (source redacted)");
        }
        _ => panic!("integration setup/task failed or exceeded 45 seconds"),
    }
}
