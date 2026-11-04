use crate::media::RedbMediaRepository;
use recondo_domain::model::{Collection, normalize_collection_name};
use recondo_domain::repository::CollectionRepository;
use redb::{Database, Error, ReadableDatabase, ReadableTable, TableDefinition};
use serde_json;

pub const COLLECTIONS: TableDefinition<&str, &str> = TableDefinition::new("COLLECTIONS");

pub fn init_collection_table(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    let _ = write_txn.open_table(COLLECTIONS)?;
    write_txn.commit()?;
    Ok(())
}

pub fn save_collection(db: &Database, collection: &Collection) -> Result<(), Error> {
    let key = normalize_collection_name(&collection.name);
    let json = serde_json::to_string(collection)
        .map_err(|e| Error::Corrupted(format!("Failed to serialize collection: {e}")))?;
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(COLLECTIONS)?;
        table.insert(key.as_str(), json.as_str())?;
    }
    write_txn.commit()?;
    Ok(())
}

pub fn get_collection(db: &Database, name: &str) -> Result<Option<Collection>, Error> {
    let key = normalize_collection_name(name);
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(COLLECTIONS)?;
    let Some(value) = table.get(key.as_str())? else {
        return Ok(None);
    };
    let collection: Collection = serde_json::from_str(value.value())
        .map_err(|e| Error::Corrupted(format!("Failed to deserialize collection: {e}")))?;
    Ok(Some(collection))
}

pub fn list_collections(db: &Database) -> Result<Vec<Collection>, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(COLLECTIONS)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (_k, v) = entry?;
        let collection: Collection = serde_json::from_str(v.value())
            .map_err(|e| Error::Corrupted(format!("Failed to deserialize collection: {e}")))?;
        out.push(collection);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn get_collections_by_prefix(
    db: &Database,
    prefix: &str,
    limit: Option<usize>,
) -> Result<Option<Vec<Collection>>, Error> {
    if limit == Some(0) {
        return Ok(None);
    }
    let normalized = normalize_collection_name(prefix);
    if normalized.is_empty() {
        return Ok(None);
    }
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(COLLECTIONS)?;
    let mut out = Vec::new();
    for entry in table.range(normalized.as_str()..)? {
        let (key, value) = entry?;
        let key_str = key.value();
        if key_str.starts_with(&normalized) {
            let collection: Collection = serde_json::from_str(value.value())
                .map_err(|e| Error::Corrupted(format!("Failed to deserialize collection: {e}")))?;
            out.push(collection);
            if limit.is_some_and(|lim| out.len() >= lim) {
                break;
            }
        } else {
            break;
        }
    }
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

pub fn delete_collection(db: &Database, name: &str) -> Result<bool, Error> {
    let key = normalize_collection_name(name);
    let write_txn = db.begin_write()?;
    let removed = {
        let mut table = write_txn.open_table(COLLECTIONS)?;
        table.remove(key.as_str())?.is_some()
    };
    write_txn.commit()?;
    Ok(removed)
}

impl CollectionRepository for RedbMediaRepository {
    type Error = Error;

    fn init(&self) -> Result<(), Self::Error> {
        init_collection_table(self.database())
    }

    fn save_collection(&self, collection: &Collection) -> Result<(), Self::Error> {
        save_collection(self.database(), collection)
    }

    fn get_collection(&self, name: &str) -> Result<Option<Collection>, Self::Error> {
        get_collection(self.database(), name)
    }

    fn list_collections(&self) -> Result<Vec<Collection>, Self::Error> {
        list_collections(self.database())
    }

    fn get_collections_by_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<Collection>>, Self::Error> {
        get_collections_by_prefix(self.database(), prefix, limit)
    }

    fn delete_collection(&self, name: &str) -> Result<bool, Self::Error> {
        delete_collection(self.database(), name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recondo_domain::model::Collection;
    use recondo_domain::repository::CollectionRepository;
    use std::fs;
    use std::path::PathBuf;

    fn temp_base(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!(
            "recondo_coll_{}_{}_{}",
            name,
            std::process::id(),
            unique
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn make_collection(name: &str) -> Collection {
        Collection {
            name: name.to_string(),
            operation: "and".to_string(),
            tag_ids: vec!["tag1".to_string()],
            groups: vec![],
            limit: 10,
        }
    }

    #[test]
    fn save_and_get_collection() {
        let base = temp_base("x");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_collection_table(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let collection = make_collection("My Collection");
        repo.save_collection(&collection).unwrap();

        let fetched = repo.get_collection("my collection").unwrap().unwrap();
        assert_eq!(fetched.name, "My Collection");
        assert_eq!(fetched.tag_ids, vec!["tag1".to_string()]);
    }

    #[test]
    fn list_and_prefix() {
        let base = temp_base("x");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_collection_table(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        repo.save_collection(&make_collection("alpha")).unwrap();
        repo.save_collection(&make_collection("beta")).unwrap();
        repo.save_collection(&make_collection("alphabet")).unwrap();

        let all = repo.list_collections().unwrap();
        assert_eq!(all.len(), 3);

        let prefixed = repo.get_collections_by_prefix("alp", None).unwrap().unwrap();
        assert_eq!(prefixed.len(), 2);

        let limited = repo.get_collections_by_prefix("alp", Some(1)).unwrap().unwrap();
        assert_eq!(limited.len(), 1);

        assert!(repo.get_collections_by_prefix("zzz", None).unwrap().is_none());
        assert!(repo.get_collections_by_prefix("", None).unwrap().is_none());
    }

    #[test]
    fn delete_collection() {
        let base = temp_base("x");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_collection_table(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        repo.save_collection(&make_collection("to delete")).unwrap();
        assert!(repo.delete_collection("to delete").unwrap());
        assert!(!repo.delete_collection("to delete").unwrap());
        assert!(repo.get_collection("to delete").unwrap().is_none());
    }

    #[test]
    fn upsert_collection() {
        let base = temp_base("x");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        init_collection_table(&db).unwrap();
        let repo = RedbMediaRepository::new(db);

        let mut collection = make_collection("same");
        repo.save_collection(&collection).unwrap();
        collection.limit = 25;
        repo.save_collection(&collection).unwrap();

        let fetched = repo.get_collection("same").unwrap().unwrap();
        assert_eq!(fetched.limit, 25);
        assert_eq!(repo.list_collections().unwrap().len(), 1);
    }

    #[test]
    fn init_via_media_init() {
        let base = temp_base("x");
        let db_path = base.join("test.redb");
        let db = Database::create(&db_path).unwrap();
        crate::media::init_db(&db).unwrap();
        let repo = RedbMediaRepository::new(db);
        repo.save_collection(&make_collection("via init")).unwrap();
        assert!(repo.get_collection("via init").unwrap().is_some());
    }
}