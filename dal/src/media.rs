use recondo_domain::model::{Media, MediaId, Tag, normalize_relationship};
use recondo_domain::repository::{MediaRepository, SaveMediaError};
use redb::{Database, Error, ReadableDatabase, ReadableTable, TableDefinition};
use std::fs;
use std::path::Path;

use crate::tag::{self, TAG, TAG_ADJ_LST};
pub use crate::tag::{
    AddTagEdgeError, RelationshipSetError, get_relationships_by_prefix, get_tag_adj_list,
    get_tags_by_relationship, upsert_relationship_tag_txn,
};

// Relationship, Tag ID
type EdgeFrmMedia = (String, String);

const MEDIA: TableDefinition<&str, &str> = TableDefinition::new("MEDIA");
pub(super) const MEDIA_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmMedia>> =
    TableDefinition::new("MEDIA_ADJ_LST");

pub fn init_db(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    let _ = write_txn.open_table(MEDIA)?;
    let _ = write_txn.open_table(MEDIA_ADJ_LST)?;
    let _ = write_txn.open_table(TAG_ADJ_LST)?;
    let _ = write_txn.open_table(TAG)?;
    let _ = write_txn.open_table(tag::RELATIONSHIP_TO_TAGS)?;
    write_txn.commit()?;
    Ok(())
}

pub fn does_media_exist(db: &Database, id: MediaId) -> Result<bool, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEDIA_ADJ_LST)?;
    let value = table.get(id.as_str())?;
    Ok(value.is_some())
}

pub fn insert_media_record_txn(txn: &redb::WriteTransaction, media: &Media) -> Result<(), Error> {
    let json = serde_json::to_string(media)
        .map_err(|e| Error::Corrupted(format!("Failed to serialize media: {e}")))?;
    let mut table = txn.open_table(MEDIA)?;
    table.insert(media.id.as_str(), json.as_str())?;
    Ok(())
}

pub fn put_media_record(db: &Database, media: &Media) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    insert_media_record_txn(&write_txn, media)?;
    write_txn.commit()?;
    Ok(())
}

pub fn get_media_by_id(db: &Database, id: &MediaId) -> Result<Option<Media>, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEDIA)?;
    let Some(value) = table.get(id.as_str())? else {
        return Ok(None);
    };
    let media: Media = serde_json::from_str(value.value())
        .map_err(|e| Error::Corrupted(format!("Failed to deserialize media: {e}")))?;
    Ok(Some(media))
}

pub fn get_media_adj_list(db: &Database, id: &MediaId) -> Result<Option<Vec<EdgeFrmMedia>>, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEDIA_ADJ_LST)?;
    let Some(value) = table.get(id.as_str())? else {
        return Ok(None);
    };
    Ok(Some(value.value()))
}

pub struct RedbMediaRepository {
    db: Database,
}

impl RedbMediaRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }
}

fn persist_media_file_with_tags(
    db: &Database,
    media: &Media,
    src_path: &Path,
    dest_path: &Path,
    tags: &[(String, Tag)],
) -> Result<(), SaveMediaError<Error>> {
    let media_id = dest_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            SaveMediaError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination path has no file stem",
            ))
        })?
        .to_string();

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(SaveMediaError::Io)?;
    }
    fs::copy(src_path, dest_path).map_err(SaveMediaError::Io)?;

    let write_txn = db
        .begin_write()
        .map_err(|error| SaveMediaError::Repository(error.into()))?;
    {
        insert_media_record_txn(&write_txn, media).map_err(SaveMediaError::Repository)?;
        let mut media_table = write_txn
            .open_table(MEDIA_ADJ_LST)
            .map_err(|error| SaveMediaError::Repository(error.into()))?;
        let mut tag_table = write_txn
            .open_table(TAG_ADJ_LST)
            .map_err(|error| SaveMediaError::Repository(error.into()))?;
        let mut tag_name_table = write_txn
            .open_table(TAG)
            .map_err(|error| SaveMediaError::Repository(error.into()))?;

        for (relationship, tag) in tags {
            let relationship = normalize_relationship(relationship);
            if relationship.is_empty() {
                return Err(SaveMediaError::Tag(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "relationship must not be empty",
                ))));
            }
            let media_edge = (relationship.clone(), tag.id().to_string());
            let existing_media = media_table
                .get(media_id.as_str())
                .map_err(|error| SaveMediaError::Repository(error.into()))?
                .map(|value| value.value())
                .unwrap_or_default();
            if !existing_media.contains(&media_edge) {
                let mut edges = existing_media;
                edges.push(media_edge);
                media_table
                    .insert(media_id.as_str(), edges)
                    .map_err(|error| SaveMediaError::Repository(error.into()))?;
            }

            let tag_edge = (relationship.clone(), media_id.clone());
            let existing_tag = tag_table
                .get(tag.id())
                .map_err(|error| SaveMediaError::Repository(error.into()))?
                .map(|value| value.value())
                .unwrap_or_default();
            if !existing_tag.contains(&tag_edge) {
                let mut edges = existing_tag;
                edges.push(tag_edge);
                tag_table
                    .insert(tag.id(), edges)
                    .map_err(|error| SaveMediaError::Repository(error.into()))?;
            }

            tag_name_table
                .insert(tag.id(), tag.name())
                .map_err(|error| SaveMediaError::Repository(error.into()))?;
            upsert_relationship_tag_txn(&write_txn, &relationship, tag.id()).map_err(|error| {
                match error {
                    RelationshipSetError::EmptyRelationship => {
                        SaveMediaError::Tag(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "relationship must not be empty",
                        )))
                    }
                    RelationshipSetError::Repository(error) => SaveMediaError::Repository(error),
                }
            })?;
        }
    }
    write_txn.commit().map_err(|error| {
        let _ = fs::remove_file(dest_path);
        SaveMediaError::Repository(error.into())
    })?;
    Ok(())
}

fn default_upload_tag() -> (String, Tag) {
    (
        normalize_relationship("to do"),
        Tag::new("needs tags").expect("the default upload tag is non-empty"),
    )
}

fn apply_upload_tags_to_duplicate(
    db: &Database,
    media_id: &MediaId,
    tags: &[(String, Tag)],
) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    {
        let (default_relationship, default_tag) = default_upload_tag();
        let default_edge = (default_relationship.clone(), default_tag.id().to_string());
        let default_tag_edge = (default_relationship.clone(), media_id.as_str().to_string());
        let mut media_table = write_txn.open_table(MEDIA_ADJ_LST)?;
        let mut media_edges = media_table
            .get(media_id.as_str())?
            .ok_or_else(|| Error::Corrupted("duplicate media has no tag adjacency list".into()))?
            .value();

        if media_edges.len() == 1 && media_edges.first() == Some(&default_edge) {
            media_edges.clear();
            media_table.insert(media_id.as_str(), media_edges)?;

            let mut tag_table = write_txn.open_table(TAG_ADJ_LST)?;
            let mut default_tag_edges = tag_table
                .get(default_tag.id())?
                .map(|value| value.value())
                .unwrap_or_default();
            default_tag_edges.retain(|edge| edge != &default_tag_edge);
            tag_table.insert(default_tag.id(), default_tag_edges)?;
        }

        let mut tag_table = write_txn.open_table(TAG_ADJ_LST)?;
        let mut tag_name_table = write_txn.open_table(TAG)?;
        for (relationship, tag) in tags {
            let relationship = normalize_relationship(relationship);
            if relationship.is_empty() {
                return Err(Error::Corrupted(
                    "upload tag relationship must not be empty".into(),
                ));
            }
            let media_edge = (relationship.clone(), tag.id().to_string());
            let mut media_edges = media_table
                .get(media_id.as_str())?
                .ok_or_else(|| {
                    Error::Corrupted("duplicate media has no tag adjacency list".into())
                })?
                .value();
            if !media_edges.contains(&media_edge) {
                media_edges.push(media_edge);
                media_table.insert(media_id.as_str(), media_edges)?;
            }

            let tag_edge = (relationship.clone(), media_id.as_str().to_string());
            let mut tag_edges = tag_table
                .get(tag.id())?
                .map(|value| value.value())
                .unwrap_or_default();
            if !tag_edges.contains(&tag_edge) {
                tag_edges.push(tag_edge);
                tag_table.insert(tag.id(), tag_edges)?;
            }
            tag_name_table.insert(tag.id(), tag.name())?;
            upsert_relationship_tag_txn(&write_txn, &relationship, tag.id()).map_err(|error| {
                match error {
                    RelationshipSetError::EmptyRelationship => {
                        Error::Corrupted("upload tag relationship must not be empty".into())
                    }
                    RelationshipSetError::Repository(error) => error,
                }
            })?;
        }
    }
    write_txn.commit()?;
    Ok(())
}

impl MediaRepository for RedbMediaRepository {
    type Error = Error;

    fn init(&self) -> Result<(), Self::Error> {
        init_db(&self.db)
    }

    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error> {
        does_media_exist(&self.db, id.clone())
    }

    fn save_media(
        &self,
        media: &Media,
        src_path: &Path,
        dest_path: &Path,
    ) -> Result<(), SaveMediaError<Self::Error>> {
        persist_media_file_with_tags(
            &self.db,
            media,
            src_path,
            dest_path,
            &[default_upload_tag()],
        )
    }

    fn save_media_with_tags(
        &self,
        media: &Media,
        src_path: &Path,
        dest_path: &Path,
        tags: &[(String, Tag)],
    ) -> Result<(), SaveMediaError<Self::Error>> {
        persist_media_file_with_tags(&self.db, media, src_path, dest_path, tags)
    }

    fn apply_upload_tags_to_duplicate(
        &self,
        media_id: &MediaId,
        tags: &[(String, Tag)],
    ) -> Result<(), Self::Error> {
        if tags.is_empty() {
            return Ok(());
        }
        apply_upload_tags_to_duplicate(&self.db, media_id, tags)
    }

    fn save_media_record(&self, media: &Media) -> Result<(), Self::Error> {
        put_media_record(&self.db, media)
    }

    fn get_media(&self, id: &MediaId) -> Result<Option<Media>, Self::Error> {
        get_media_by_id(&self.db, id)
    }

    fn get_media_adj_list(
        &self,
        id: &MediaId,
    ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        get_media_adj_list(&self.db, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::RELATIONSHIP_TO_TAGS;
    use recondo_domain::repository::{MediaRepository, TagRepository};
    use std::fs;

    fn temp_base(name: &str) -> std::path::PathBuf {
        let base =
            std::env::temp_dir().join(format!("recondo_dal_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn make_test_media(id_str: &str) -> Media {
        Media::new(
            MediaId::from_hash(id_str).unwrap(),
            url::Url::parse(&format!("file:///assets/{id_str}.txt")).unwrap(),
            None,
        )
    }

    #[test]
    fn test_save_media_copies_and_inserts() {
        let base = temp_base("copy");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"hello").unwrap();

        let db_path = base.join("test.redb");
        let _ = fs::remove_file(&db_path);
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"hello");

        let read_txn = repo.database().begin_read().unwrap();
        let media_table = read_txn.open_table(MEDIA_ADJ_LST).unwrap();
        let media_edge = media_table.get("hash123").unwrap().unwrap();
        let tag_table = read_txn.open_table(TAG_ADJ_LST).unwrap();
        let expected_tag = Tag::new("needs tags").unwrap().id().to_string();
        let tag = tag_table.get(expected_tag.as_str()).unwrap().unwrap();
        assert_eq!(
            media_edge.value(),
            vec![("to do".to_string(), expected_tag.to_string())]
        );
        assert_eq!(
            tag.value(),
            vec![("to do".to_string(), "hash123".to_string())]
        );

        let fetched = repo
            .get_media(&MediaId::from_hash("hash123").unwrap())
            .unwrap()
            .expect("media record should exist in MEDIA table");
        assert_eq!(fetched.id.as_str(), "hash123");
        assert_eq!(fetched.url, media.url);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_save_media_shared_tag_accumulates_media() {
        let base = temp_base("shared_tag");
        let src_a = base.join("a.txt");
        let src_b = base.join("b.txt");
        fs::write(&src_a, b"a").unwrap();
        fs::write(&src_b, b"b").unwrap();

        let db_path = base.join("test.redb");
        let _ = fs::remove_file(&db_path);
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media_a = make_test_media("hash_a");
        let media_b = make_test_media("hash_b");
        repo.save_media(&media_a, &src_a, &base.join("assets").join("hash_a.txt"))
            .unwrap();
        repo.save_media(&media_b, &src_b, &base.join("assets").join("hash_b.txt"))
            .unwrap();

        let read_txn = repo.database().begin_read().unwrap();
        let tag_table = read_txn.open_table(TAG_ADJ_LST).unwrap();
        let expected_tag = Tag::new("needs tags").unwrap().id().to_string();
        let tag = tag_table.get(expected_tag.as_str()).unwrap().unwrap();
        assert_eq!(
            tag.value(),
            vec![
                ("to do".to_string(), "hash_a".to_string()),
                ("to do".to_string(), "hash_b".to_string()),
            ]
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_save_media_does_not_duplicate_edges() {
        let base = temp_base("no_dupes");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"hello").unwrap();

        let db_path = base.join("test.redb");
        let _ = fs::remove_file(&db_path);
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();
        repo.save_media(&media, &src, &dest).unwrap();

        let read_txn = repo.database().begin_read().unwrap();
        let media_table = read_txn.open_table(MEDIA_ADJ_LST).unwrap();
        let media_edge = media_table.get("hash123").unwrap().unwrap();
        assert_eq!(media_edge.value().len(), 1);

        let tag_table = read_txn.open_table(TAG_ADJ_LST).unwrap();
        let expected_tag = Tag::new("needs tags").unwrap().id().to_string();
        let tag = tag_table.get(expected_tag.as_str()).unwrap().unwrap();
        assert_eq!(tag.value().len(), 1);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn upload_tags_replace_only_the_default_pair_for_duplicates() {
        let base = temp_base("upload_tag_replacement");
        let src = base.join("src.txt");
        fs::write(&src, b"upload tag behavior").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let default_only = make_test_media("default_only");
        repo.save_media(
            &default_only,
            &src,
            &base.join("assets").join("default_only.txt"),
        )
        .unwrap();
        let selected = ("occasion".to_string(), Tag::new("birthday").unwrap());
        repo.apply_upload_tags_to_duplicate(&default_only.id, std::slice::from_ref(&selected))
            .unwrap();
        assert_eq!(
            repo.get_media_adj_list(&default_only.id).unwrap().unwrap(),
            vec![("occasion".to_string(), selected.1.id().to_string())]
        );

        let existing_pair = ("place".to_string(), Tag::new("kitchen").unwrap());
        let tagged_media = make_test_media("already_tagged");
        repo.save_media_with_tags(
            &tagged_media,
            &src,
            &base.join("assets").join("already_tagged.txt"),
            std::slice::from_ref(&existing_pair),
        )
        .unwrap();
        repo.apply_upload_tags_to_duplicate(&tagged_media.id, std::slice::from_ref(&selected))
            .unwrap();
        assert_eq!(
            repo.get_media_adj_list(&tagged_media.id).unwrap().unwrap(),
            vec![
                ("place".to_string(), existing_pair.1.id().to_string()),
                ("occasion".to_string(), selected.1.id().to_string()),
            ]
        );

        let read_txn = repo.database().begin_read().unwrap();
        let tag_table = read_txn.open_table(TAG_ADJ_LST).unwrap();
        let default_tag = Tag::new("needs tags").unwrap();
        let default_edges = tag_table.get(default_tag.id()).unwrap().unwrap().value();
        assert!(
            !default_edges
                .iter()
                .any(|edge| edge.1 == default_only.id.as_str())
        );
        assert!(
            !default_edges
                .iter()
                .any(|edge| edge.1 == tagged_media.id.as_str())
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_save_media_missing_source() {
        let base = temp_base("missing");
        let src = base.join("nope.txt");
        let dest = base.join("assets").join("hash123.txt");

        let db_path = base.join("test.redb");
        let _ = fs::remove_file(&db_path);
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        let result = repo.save_media(&media, &src, &dest);
        assert!(matches!(result, Err(SaveMediaError::Io(_))));
        assert!(!dest.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_save_media_creates_parent_dir() {
        let base = temp_base("parent");
        let src = base.join("src.bin");
        let dest = base.join("nested").join("deep").join("hash999.bin");
        fs::write(&src, b"data").unwrap();

        let db_path = base.join("test.redb");
        let _ = fs::remove_file(&db_path);
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash999");
        repo.save_media(&media, &src, &dest).unwrap();
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"data");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_put_media_record_and_get_media() {
        let base = temp_base("put_get_media");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let id = MediaId::from_hash("record123").unwrap();
        let url = url::Url::parse("file:///assets/record123.png").unwrap();
        let mut media = Media::new(
            id.clone(),
            url.clone(),
            Some(recondo_domain::model::MediaOpts {
                title: Some("Title 1".to_string()),
                mime_type: Some("image/png".to_string()),
                file_size: Some(2048),
                view_count: Some(10),
            }),
        );

        // Insert
        repo.save_media_record(&media).unwrap();

        let fetched = repo.get_media(&id).unwrap().expect("media should exist");
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.title, Some("Title 1".to_string()));
        assert_eq!(fetched.view_count, 10);

        // Update
        media.title = Some("Updated Title".to_string());
        media.view_count = 11;
        repo.save_media_record(&media).unwrap();

        let updated = repo.get_media(&id).unwrap().expect("media should exist");
        assert_eq!(updated.title, Some("Updated Title".to_string()));
        assert_eq!(updated.view_count, 11);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_add_tag_edge_rejects_missing_media() {
        let base = temp_base("missing_media_tag");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        assert!(matches!(
            repo.add_tag_edge("missing", &Tag::new("tag").unwrap(), "liked"),
            Err(AddTagEdgeError::MediaNotFound)
        ));
        assert!(
            !repo
                .media_exists(&MediaId::from_hash("missing").unwrap())
                .unwrap()
        );
        assert!(!repo.tag_exists("tag-id").unwrap());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_get_adj_lists() {
        let base = temp_base("get_adj_lists");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"test").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();

        let tag_id = Tag::new("needs tags").unwrap().id().to_string();

        // Check Media adj list
        let media_adj = repo.get_media_adj_list(&media.id).unwrap();
        assert_eq!(media_adj, Some(vec![("to do".to_string(), tag_id.clone())]));

        // Check Tag adj list
        let tag_adj = repo.get_tag_adj_list(&tag_id).unwrap();
        assert_eq!(
            tag_adj,
            Some(vec![("to do".to_string(), "hash123".to_string())])
        );

        // Not found cases
        let missing_media = MediaId::from_hash("not_here").unwrap();
        assert_eq!(repo.get_media_adj_list(&missing_media).unwrap(), None);
        assert_eq!(repo.get_tag_adj_list("not_here").unwrap(), None);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_relationship_set_populated_on_save() {
        let base = temp_base("relationship_set_save");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"test").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();

        let tag_id = Tag::new("needs tags").unwrap().id().to_string();

        let read_txn = repo.database().begin_read().unwrap();
        let rel_table = read_txn.open_table(RELATIONSHIP_TO_TAGS).unwrap();
        let rel = rel_table.get("to do").unwrap().unwrap();
        assert_eq!(rel.value(), vec![tag_id]);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_relationship_set_dedup_on_save() {
        let base = temp_base("relationship_set_dedup_save");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"test").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();
        repo.save_media(&media, &src, &dest).unwrap();

        let tag_id = Tag::new("needs tags").unwrap().id().to_string();

        let read_txn = repo.database().begin_read().unwrap();
        let rel_table = read_txn.open_table(RELATIONSHIP_TO_TAGS).unwrap();
        let rel = rel_table.get("to do").unwrap().unwrap();
        assert_eq!(rel.value(), vec![tag_id]);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_add_tag_edge_updates_relationship_set() {
        let base = temp_base("relationship_set_edge");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"test").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();

        let tag = Tag::new("landscape").unwrap();
        repo.add_tag_edge("hash123", &tag, "LIKED").unwrap();
        repo.add_tag_edge("hash123", &tag, "LIKED").unwrap();

        let read_txn = repo.database().begin_read().unwrap();
        let rel_table = read_txn.open_table(RELATIONSHIP_TO_TAGS).unwrap();
        let liked = rel_table.get("liked").unwrap().unwrap();
        assert_eq!(liked.value(), vec![tag.id().to_string()]);

        let todo = rel_table.get("to do").unwrap().unwrap();
        assert_eq!(
            todo.value(),
            vec![Tag::new("needs tags").unwrap().id().to_string()]
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_upsert_relationship_tag_txn_normalization_and_empty() {
        let base = temp_base("relationship_normalize");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();

        let write_txn = db.begin_write().unwrap();
        upsert_relationship_tag_txn(&write_txn, "  TO DO  ", "tag1").unwrap();
        upsert_relationship_tag_txn(&write_txn, "to do", "tag1").unwrap();
        upsert_relationship_tag_txn(&write_txn, "To Do", "tag2").unwrap();
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let rel_table = read_txn.open_table(RELATIONSHIP_TO_TAGS).unwrap();
        let rel = rel_table.get("to do").unwrap().unwrap();
        assert_eq!(rel.value(), vec!["tag1".to_string(), "tag2".to_string()]);

        let write_txn = db.begin_write().unwrap();
        let err = upsert_relationship_tag_txn(&write_txn, "   ", "tag3").unwrap_err();
        assert!(matches!(err, RelationshipSetError::EmptyRelationship));
        write_txn.abort().unwrap();

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_get_relationships_by_prefix() {
        let base = temp_base("relationships_prefix");
        let src = base.join("src.txt");
        let dest = base.join("assets").join("hash123.txt");
        fs::write(&src, b"test").unwrap();

        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let media = make_test_media("hash123");
        repo.save_media(&media, &src, &dest).unwrap();

        let tag1 = Tag::new("tag1").unwrap();
        let tag2 = Tag::new("tag2").unwrap();
        let tag3 = Tag::new("tag3").unwrap();

        repo.add_tag_edge("hash123", &tag1, "total recall").unwrap();
        repo.add_tag_edge("hash123", &tag2, "topic").unwrap();
        repo.add_tag_edge("hash123", &tag3, "other").unwrap();
        // Note: save_media automatically creates a "to do" edge with tag "needs tags"

        let matches = repo
            .get_relationships_by_prefix("to", None)
            .unwrap()
            .unwrap();
        assert_eq!(matches, vec!["to do", "topic", "total recall"]);

        let matches_limited = repo
            .get_relationships_by_prefix("to", Some(2))
            .unwrap()
            .unwrap();
        assert_eq!(matches_limited, vec!["to do", "topic"]);

        let matches_zero = repo.get_relationships_by_prefix("to", Some(0)).unwrap();
        assert_eq!(matches_zero, None);

        let matches_total = repo
            .get_relationships_by_prefix("tot", None)
            .unwrap()
            .unwrap();
        assert_eq!(matches_total, vec!["total recall"]);

        let no_matches = repo.get_relationships_by_prefix("zeta", None).unwrap();
        assert_eq!(no_matches, None);

        let _ = fs::remove_dir_all(&base);
    }
}
