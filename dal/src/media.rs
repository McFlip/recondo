use recondo_domain::model::MediaId;
use recondo_domain::repository::MediaRepository;
use redb::{Database, Error, ReadableDatabase, TableDefinition};

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
}

impl MediaRepository for RedbMediaRepository {
    type Error = Error;

    fn init(&self) -> Result<(), Self::Error> {
        init_db(&self.db)
    }

    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error> {
        does_media_exist(&self.db, id.clone())
    }
}
