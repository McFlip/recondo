use recondo_domain::model::MediaId;
use recondo_domain::repository::{MediaRepository, SaveMediaError};
use redb::{Database, Error, ReadableDatabase, TableDefinition};
use std::fs;
use std::path::Path;

type EdgeFrmMedia = (String, String);

const MEDIA_ADJ_LST: TableDefinition<&str, EdgeFrmMedia> = TableDefinition::new("MEDIA_ADJ_LST");

pub fn init_db(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    let _ = write_txn.open_table(MEDIA_ADJ_LST)?;
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

        let write_txn = self.db.begin_write().map_err(|e| SaveMediaError::Repository(e.into()))?;
        {
            let mut table = write_txn.open_table(MEDIA_ADJ_LST).map_err(|e| SaveMediaError::Repository(e.into()))?;
            table
                .insert(hash.as_str(), ("to do".to_string(), "needs tags".to_string()))
                .map_err(|e| SaveMediaError::Repository(e.into()))?;
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
        let base = std::env::temp_dir().join(format!("recondo_dal_{}_{}", name, std::process::id()));
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
        let table = read_txn.open_table(MEDIA_ADJ_LST).unwrap();
        let v = table.get("hash123").unwrap().unwrap();
        assert_eq!(v.value(), ("to do".to_string(), "needs tags".to_string()));

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