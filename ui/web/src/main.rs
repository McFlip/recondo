use std::fs;
use std::path::PathBuf;

use recondo_dal::media::RedbMediaRepository;
use recondo_domain::cmd;
use recondo_domain::repository::MediaRepository;
use redb::Database;
use redb::Error;

fn main() -> Result<(), Error> {
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("db")
        .join("graph.redb");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let db = Database::create(&db_path)?;
    let repo = RedbMediaRepository::new(db);

    repo.init()?;

    let upload_path = PathBuf::from("fubar.jpg");
    match cmd::create_media_from_file(upload_path, &repo)? {
        Some(media) => println!("Created media: {:?}", media),
        None => println!("Media already exists"),
    }

    Ok(())
}
