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

    let upload_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data_trap.png");
    let media = cmd::create_media_from_file(upload_path, &repo)
        .unwrap()
        .unwrap();
    let tag1 =
        cmd::add_tag_to_media(media.id.as_str(), "new tag", "new relationship", &repo).unwrap();
    let to_do = &qry::get_relationships_by_prefix("to", Some(1), &repo)
        .unwrap()
        .unwrap()[0];
    let tag0 = &qry::get_tags_by_relationship(to_do, &repo)
        .unwrap()
        .unwrap()[0];
    let upload_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exploits_of_a_mom.png");
    let media2 = cmd::create_media_from_file(upload_path, &repo)
        .unwrap()
        .unwrap();
    let tag2 =
        cmd::add_tag_to_media(media2.id.as_str(), "bobby tables", "sql memes", &repo).unwrap();

    let media_results = qry::get_media_ids_by_logical_and([tag0.id()], &repo).unwrap();
    let media_id = media_results.iter().last().unwrap();
    let final_result = qry::get_media_by_id(media_id, &repo);
    println!("Result of 'needs tags': {:?}", final_result);
    println!("---");

    let media_results = qry::get_media_ids_by_logical_and([tag0.id(), tag1.id()], &repo).unwrap();
    let media_id = media_results.iter().last().unwrap();
    let final_result = qry::get_media_by_id(media_id, &repo);
    println!("Result of 'needs tags' && 'new tag': {:?}", final_result);
    println!("---");

    let media_results = qry::get_media_ids_by_logical_and([tag1.id(), tag2.id()], &repo).unwrap();
    let media_id = media_results.iter().last();
    println!("Result of 'needs tags' && 'bobby tables': {:?}", media_id);
    println!("---");

    let media_results = qry::get_media_ids_by_logical_or([tag1.id(), tag2.id()], &repo).unwrap();
    let media_id = media_results;
    println!("Result of 'needs tags' || 'bobby tables': {:?}", media_id);
    println!("---");

    let media_results = qry::get_media_ids_by_logical_or_then_and(
        [[tag1.id(), tag2.id()], [tag0.id(), tag0.id()]],
        &repo,
    )
    .unwrap();
    let media_id = media_results;
    println!(
        "Result of ('new tag' || 'bobby tables') && 'needs tags': {:?}",
        media_id
    );
    println!("---");

    let media_results = qry::get_media_ids_by_logical_and_then_or(
        [[tag1.id(), tag2.id()], [tag0.id(), tag0.id()]],
        &repo,
    )
    .unwrap();
    let media_id = media_results;
    println!(
        "Result of ('new tag' && 'bobby tables') || 'needs tags': {:?}",
        media_id
    );

    Ok(())
}
