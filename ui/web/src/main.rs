mod app;
mod components;
mod layout;
mod pages;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use recondo_dal::media::RedbMediaRepository;
use recondo_domain::repository::MediaRepository;
use redb::Database;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let db_path =
        std::env::var("RECONDO_DB_PATH").unwrap_or_else(|_| "ui/web/db/graph.redb".to_string());
    if let Some(parent) = Path::new(&db_path).parent() {
        fs::create_dir_all(parent)?;
    }

    let db = Database::create(&db_path)?;
    let repo = RedbMediaRepository::new(db);
    repo.init()?;
    let state = app::AppState {
        repo: Arc::new(repo),
        assets_dir: std::path::PathBuf::from("assets"),
    };

    let address = std::env::var("RECONDO_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = TcpListener::bind(&address).await?;

    println!("Recondo web UI listening on http://{address}");
    axum::serve(listener, app::router(state)).await?;

    Ok(())
}
