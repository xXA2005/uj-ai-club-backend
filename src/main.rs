use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use uj_ai_club_backend::create_app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let pg_user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "uj_ai_club".to_string());
        let pg_pass = std::env::var("POSTGRES_PASSWORD").unwrap();
        let pg_db = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "uj_ai_club".to_string());
        let pg_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres".to_string());

        format!("postgres://{pg_user}:{pg_pass}@{pg_host}:5432/{pg_db}")
    });
    let server_addr =
        std::env::var("SERVER_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8000".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let app = create_app(pool);

    let addr: SocketAddr = server_addr.parse()?;

    tracing::info!("Starting server on {} yo", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
