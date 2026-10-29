mod app;
mod components;
mod layout;
mod pages;

use std::error::Error;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = std::env::var("RECONDO_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = TcpListener::bind(&address).await?;

    println!("Recondo web UI listening on http://{address}");
    axum::serve(listener, app::router()).await?;

    Ok(())
}
