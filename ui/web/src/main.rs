use std::error::Error as StdError;
use std::fs;
use std::path::PathBuf;

use recondo_dal::media::RedbMediaRepository;
use recondo_domain::repository::{MediaRepository, TagRepository};
use recondo_domain::{cmd, qry};
use redb::Database;

// This application is temporarily only used for smoke testing while the core domain is being
// developed.

fn main() -> Result<(), Box<dyn StdError>> {
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("db")
        .join("graph.redb");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let db = Database::create(&db_path)?;
    let repo = RedbMediaRepository::new(db);

    MediaRepository::init(&repo)?;
    TagRepository::init(&repo)?;

    let upload_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    match cmd::create_media_from_file(upload_path, &repo)? {
        Some(media) => {
            println!("Created media: {:?}", media);
            match cmd::add_tag_to_media(media.id.as_str(), "new tag", "new relationship", &repo) {
                Ok(tag) => println!("Added tag: {:?}", tag),
                Err(e) => println!("Add tag error: {}", e),
            }
            if let Some(fetched) = qry::get_media_by_id(&media.id, &repo)? {
                println!("Retrieved media from DB: {:?}", fetched);
            }
        }
        None => println!("Media already exists"),
    }

    match qry::get_relationships_by_prefix("new relationship", None, &repo)? {
        Some(relationship) => {
            println!("Found relationship: {:?}", relationship)
        }
        None => {
            eprintln!("Failed to find relationship 'new relationship'")
        }
    }

    match qry::get_tags_by_relationship("new relationship", &repo)? {
        Some(tags) => {
            println!("Found tags for 'new relationship': {:?}", tags)
        }
        None => {
            eprintln!("Failed to find tags for 'new relationship'")
        }
    }

    Ok(())
}
