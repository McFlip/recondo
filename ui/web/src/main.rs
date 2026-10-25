use std::fs;
use std::path::PathBuf;

use recondo_dal::media;
use recondo_domain::cmd;
use redb::Database;
use redb::Error;

fn main() -> Result<(), Error> {
    // Build database path relative to the crate (ui/web)
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("db").join("graph.redb");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create/open the database
    let db = Database::create(&db_path)?;

    // Initialize tables if they don't exist
    media::init_db(&db)?;

    // Create media via domain command
    let upload_path = PathBuf::from("fubar.jpg");
    let media = cmd::create_media_from_file(upload_path);

    // Check if media exists via DAL
    let media_id = media.id.clone();
    let exists = media::does_media_exist(&db, media.id)?;
    println!("Media {:?} exists: {}", media_id, exists);

    Ok(())
}
