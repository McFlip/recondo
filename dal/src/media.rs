use recondo_domain::model::{Media, MediaId, Tag, normalize_relationship};
use recondo_domain::repository::{MediaRepository, SaveMediaError, TagRepository};
use redb::{Database, Error, ReadableDatabase, ReadableTable, TableDefinition};
use std::fs;
use std::path::Path;

// Relationship, Tag ID
type EdgeFrmMedia = (String, String);
// Relationship, Media ID
type EdgeFrmTag = (String, String);

const MEDIA: TableDefinition<&str, &str> = TableDefinition::new("MEDIA");
const MEDIA_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmMedia>> =
    TableDefinition::new("MEDIA_ADJ_LST");
const TAG_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmTag>> = TableDefinition::new("TAG_ADJ_LST");
const RELATIONSHIP_TO_TAGS: TableDefinition<&str, Vec<String>> =
    TableDefinition::new("RELATIONSHIP_TO_TAGS");

#[derive(Debug)]
pub enum AddTagEdgeError {
    MediaNotFound,
    Repository(Box<dyn std::error::Error + Send + Sync>),
}

impl AddTagEdgeError {
    fn repository<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Repository(Box::new(error))
    }
}

impl std::fmt::Display for AddTagEdgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MediaNotFound => write!(f, "media not found"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl std::error::Error for AddTagEdgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MediaNotFound => None,
            Self::Repository(e) => Some(e.as_ref()),
        }
    }
}

pub fn init_db(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    let _ = write_txn.open_table(MEDIA)?;
    let _ = write_txn.open_table(MEDIA_ADJ_LST)?;
    let _ = write_txn.open_table(TAG_ADJ_LST)?;
    let _ = write_txn.open_table(RELATIONSHIP_TO_TAGS)?;
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

pub fn get_tag_adj_list(db: &Database, tag_id: &str) -> Result<Option<Vec<EdgeFrmTag>>, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(TAG_ADJ_LST)?;
    let Some(value) = table.get(tag_id)? else {
        return Ok(None);
    };
    Ok(Some(value.value()))
}

pub fn get_relationships_by_prefix(
    db: &Database,
    prefix: &str,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Error> {
    if limit == Some(0) {
        return Ok(None);
    }
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(RELATIONSHIP_TO_TAGS)?;
    let mut matches = Vec::new();
    for entry in table.range(prefix..)? {
        let (key, _) = entry?;
        let key_str = key.value();
        if key_str.starts_with(prefix) {
            matches.push(key_str.to_string());
            if limit.is_some_and(|lim| matches.len() >= lim) {
                break;
            }
        } else {
            break;
        }
    }
    if matches.is_empty() {
        Ok(None)
    } else {
        Ok(Some(matches))
    }
}

#[derive(Debug)]
pub enum RelationshipSetError {
    EmptyRelationship,
    Repository(Error),
}

impl RelationshipSetError {
    fn repository<E: Into<Error>>(error: E) -> Self {
        Self::Repository(error.into())
    }
}

impl std::fmt::Display for RelationshipSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRelationship => write!(f, "relationship must not be empty"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl std::error::Error for RelationshipSetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EmptyRelationship => None,
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn upsert_relationship_tag_txn(
    txn: &redb::WriteTransaction,
    relationship: &str,
    tag_id: &str,
) -> Result<(), RelationshipSetError> {
    let relationship = normalize_relationship(relationship);
    if relationship.is_empty() {
        return Err(RelationshipSetError::EmptyRelationship);
    }
    let mut table = txn
        .open_table(RELATIONSHIP_TO_TAGS)
        .map_err(RelationshipSetError::repository)?;
    let existing = table
        .get(relationship.as_str())
        .map_err(RelationshipSetError::repository)?
        .map(|value| value.value())
        .unwrap_or_default();
    if existing.contains(&tag_id.to_string()) {
        return Ok(());
    }
    let mut tags = existing;
    tags.push(tag_id.to_string());
    table
        .insert(relationship.as_str(), tags)
        .map_err(RelationshipSetError::repository)?;
    Ok(())
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
        let hash = dest_path
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

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| SaveMediaError::Repository(e.into()))?;
        {
            insert_media_record_txn(&write_txn, media).map_err(SaveMediaError::Repository)?;

            let tag = Tag::new("needs tags").map_err(|e| SaveMediaError::Tag(Box::new(e)))?;
            let relationship = normalize_relationship("to do");
            let media_edge = (relationship.clone(), tag.id().to_string());
            let tag_edge = (relationship.clone(), hash.to_string());

            let mut media_table = write_txn
                .open_table(MEDIA_ADJ_LST)
                .map_err(|e| SaveMediaError::Repository(e.into()))?;
            let existing_media = media_table
                .get(hash.as_str())
                .map_err(|e| SaveMediaError::Repository(e.into()))?
                .map(|value| value.value())
                .unwrap_or_default();
            if !existing_media.contains(&media_edge) {
                let mut edges = existing_media;
                edges.push(media_edge);
                media_table
                    .insert(hash.as_str(), edges)
                    .map_err(|e| SaveMediaError::Repository(e.into()))?;
            }

            let mut tag_table = write_txn
                .open_table(TAG_ADJ_LST)
                .map_err(|e| SaveMediaError::Repository(e.into()))?;
            let existing_tag = tag_table
                .get(tag.id())
                .map_err(|e| SaveMediaError::Repository(e.into()))?
                .map(|value| value.value())
                .unwrap_or_default();
            if !existing_tag.contains(&tag_edge) {
                let mut edges = existing_tag;
                edges.push(tag_edge);
                tag_table
                    .insert(tag.id(), edges)
                    .map_err(|e| SaveMediaError::Repository(e.into()))?;
            }

            upsert_relationship_tag_txn(&write_txn, &relationship, tag.id()).map_err(
                |e| match e {
                    RelationshipSetError::EmptyRelationship => {
                        SaveMediaError::Tag(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "relationship must not be empty",
                        )))
                    }
                    RelationshipSetError::Repository(err) => SaveMediaError::Repository(err),
                },
            )?;
        }
        write_txn.commit().map_err(|e| {
            let _ = fs::remove_file(dest_path);
            SaveMediaError::Repository(e.into())
        })?;
        Ok(())
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

impl TagRepository for RedbMediaRepository {
    type Error = AddTagEdgeError;

    fn init(&self) -> Result<(), Self::Error> {
        init_db(&self.db).map_err(AddTagEdgeError::repository)
    }

    fn tag_exists(&self, id: &str) -> Result<bool, Self::Error> {
        let read_txn = self.db.begin_read().map_err(AddTagEdgeError::repository)?;
        let table = read_txn
            .open_table(TAG_ADJ_LST)
            .map_err(AddTagEdgeError::repository)?;
        Ok(table
            .get(id)
            .map_err(AddTagEdgeError::repository)?
            .is_some())
    }

    fn add_tag_edge(&self, media_id: &str, tag_id: &str, label: &str) -> Result<(), Self::Error> {
        let label = normalize_relationship(label);
        if label.is_empty() {
            return Err(AddTagEdgeError::repository(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "label must not be empty",
            )));
        }
        let write_txn = self.db.begin_write().map_err(AddTagEdgeError::repository)?;
        {
            let media_edge = (label.clone(), tag_id.to_string());
            let mut media_table = write_txn
                .open_table(MEDIA_ADJ_LST)
                .map_err(AddTagEdgeError::repository)?;
            let existing_media = media_table
                .get(media_id)
                .map_err(AddTagEdgeError::repository)?
                .ok_or(AddTagEdgeError::MediaNotFound)?
                .value();
            if !existing_media.contains(&media_edge) {
                let mut edges = existing_media;
                edges.push(media_edge);
                media_table
                    .insert(media_id, edges)
                    .map_err(AddTagEdgeError::repository)?;
            }

            let tag_edge = (label.clone(), media_id.to_string());
            let mut tag_table = write_txn
                .open_table(TAG_ADJ_LST)
                .map_err(AddTagEdgeError::repository)?;
            let existing_tag = tag_table
                .get(tag_id)
                .map_err(AddTagEdgeError::repository)?
                .map(|value| value.value())
                .unwrap_or_default();
            if !existing_tag.contains(&tag_edge) {
                let mut edges = existing_tag;
                edges.push(tag_edge);
                tag_table
                    .insert(tag_id, edges)
                    .map_err(AddTagEdgeError::repository)?;
            }

            upsert_relationship_tag_txn(&write_txn, &label, tag_id)
                .map_err(AddTagEdgeError::repository)?;
        }
        write_txn.commit().map_err(AddTagEdgeError::repository)?;
        Ok(())
    }

    fn get_tag_adj_list(&self, tag_id: &str) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        get_tag_adj_list(&self.db, tag_id).map_err(AddTagEdgeError::repository)
    }

    fn get_relationships_by_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Self::Error> {
        get_relationships_by_prefix(&self.db, prefix, limit).map_err(AddTagEdgeError::repository)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recondo_domain::repository::MediaRepository;
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
            repo.add_tag_edge("missing", "tag-id", "liked"),
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
        repo.add_tag_edge("hash123", tag.id(), "LIKED").unwrap();
        repo.add_tag_edge("hash123", tag.id(), "LIKED").unwrap();

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

        repo.add_tag_edge("hash123", tag1.id(), "total recall")
            .unwrap();
        repo.add_tag_edge("hash123", tag2.id(), "topic").unwrap();
        repo.add_tag_edge("hash123", tag3.id(), "other").unwrap();
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
