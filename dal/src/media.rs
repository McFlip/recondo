use recondo_domain::model::{MediaId, Tag};
use recondo_domain::repository::{MediaRepository, SaveMediaError};
use redb::{Database, Error, ReadableDatabase, ReadableTable, TableDefinition};
use std::fs;
use std::path::Path;

// Relationship, Tag ID
type EdgeFrmMedia = (String, String);
// Relationship, Media ID
type EdgeFrmTag = (String, String);

const MEDIA_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmMedia>> =
    TableDefinition::new("MEDIA_ADJ_LST");
const TAG_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmTag>> = TableDefinition::new("TAG_ADJ_LST");

pub fn init_db(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    let _ = write_txn.open_table(MEDIA_ADJ_LST)?;
    let _ = write_txn.open_table(TAG_ADJ_LST)?;
    write_txn.commit()?;
    Ok(())
}

pub fn does_media_exist(db: &Database, id: MediaId) -> Result<bool, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEDIA_ADJ_LST)?;
    let value = table.get(id.as_str())?;
    Ok(value.is_some())
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
            let tag = Tag::new("needs tags").map_err(|e| SaveMediaError::Tag(Box::new(e)))?;
            let media_edge = ("to do".to_string(), tag.id().to_string());
            let tag_edge = ("to do".to_string(), hash.to_string());

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
        }
        if let Err(e) = write_txn.commit() {
            let _ = fs::remove_file(dest_path);
            return Err(SaveMediaError::Repository(e.into()));
        }
        Ok(())
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

        repo.save_media(&src, &dest).unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"hello");

        let read_txn = repo.database().begin_read().unwrap();
        let media_table = read_txn.open_table(MEDIA_ADJ_LST).unwrap();
        let media = media_table.get("hash123").unwrap().unwrap();
        let tag_table = read_txn.open_table(TAG_ADJ_LST).unwrap();
        let expected_tag = Tag::new("needs tags").unwrap().id().to_string();
        let tag = tag_table.get(expected_tag.as_str()).unwrap().unwrap();
        assert_eq!(
            media.value(),
            vec![("to do".to_string(), expected_tag.to_string())]
        );
        assert_eq!(
            tag.value(),
            vec![("to do".to_string(), "hash123".to_string())]
        );

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

        repo.save_media(&src_a, &base.join("assets").join("hash_a.txt"))
            .unwrap();
        repo.save_media(&src_b, &base.join("assets").join("hash_b.txt"))
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

        repo.save_media(&src, &dest).unwrap();
        repo.save_media(&src, &dest).unwrap();

        let read_txn = repo.database().begin_read().unwrap();
        let media_table = read_txn.open_table(MEDIA_ADJ_LST).unwrap();
        let media = media_table.get("hash123").unwrap().unwrap();
        assert_eq!(media.value().len(), 1);

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

        let result = repo.save_media(&src, &dest);
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

        repo.save_media(&src, &dest).unwrap();
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"data");

        let _ = fs::remove_dir_all(&base);
    }
}
