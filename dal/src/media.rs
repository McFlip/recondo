use recondo_domain::model::MediaId;
use redb::{Database, Error, ReadableDatabase, TableDefinition};

type EdgeFrmMedia = (String, String);

const MEDIA_ADJ_LST: TableDefinition<&str, EdgeFrmMedia> = TableDefinition::new("MEDIA_ADJ_LST");

pub fn does_media_exist(db: Database, id: MediaId) -> Result<bool, Error> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEDIA_ADJ_LST)?;
    match table.get(id.as_str()).expect("to read from MEDIA_ADJ_LST") {
        None => Ok(false),
        _ => Ok(true),
    }
}
