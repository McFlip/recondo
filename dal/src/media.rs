use recondo_domain::model::MediaId;
use redb::{Database, Error, ReadableDatabase, TableDefinition};

type EdgeFrmMedia = (String, String);

const MEDIA_ADJ_LST: TableDefinition<&str, EdgeFrmMedia> = TableDefinition::new("MEDIA_ADJ_LST");

pub fn init_db(db: &Database) -> Result<(), Error> {
    let write_txn = db.begin_write()?;
    // Opening a table in a write transaction creates it if it does not exist.
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
