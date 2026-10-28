use crate::media::{MEDIA_ADJ_LST, RedbMediaRepository};
use recondo_domain::model::{Tag, normalize_relationship};
use recondo_domain::repository::TagRepository;
use redb::{Database, Error, ReadableDatabase, ReadableTable, TableDefinition};

// Relationship, Media ID
type EdgeFrmTag = (String, String);

pub(super) const TAG_ADJ_LST: TableDefinition<&str, Vec<EdgeFrmTag>> =
    TableDefinition::new("TAG_ADJ_LST");
pub(super) const TAG: TableDefinition<&str, &str> = TableDefinition::new("TAG");
pub(super) const RELATIONSHIP_TO_TAGS: TableDefinition<&str, Vec<String>> =
    TableDefinition::new("RELATIONSHIP_TO_TAGS");

#[derive(Debug)]
pub enum AddTagEdgeError {
    MediaNotFound,
    Repository(Box<dyn std::error::Error + Send + Sync>),
}

impl AddTagEdgeError {
    pub(super) fn repository<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
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

pub fn get_tags_by_relationship(
    db: &Database,
    relationship: &str,
) -> Result<Option<Vec<Tag>>, Error> {
    let read_txn = db.begin_read()?;
    let rel_table = read_txn.open_table(RELATIONSHIP_TO_TAGS)?;
    let Some(value) = rel_table.get(relationship)? else {
        return Ok(None);
    };
    let tag_ids = value.value();
    if tag_ids.is_empty() {
        return Ok(None);
    }
    let tag_table = read_txn.open_table(TAG)?;
    let mut tags = Vec::new();
    for id in tag_ids {
        let name = tag_table
            .get(id.as_str())?
            .map(|v| v.value().to_string())
            .unwrap_or_else(|| id.clone());
        tags.push(Tag::from_parts(id.clone(), name));
    }
    if tags.is_empty() {
        Ok(None)
    } else {
        Ok(Some(tags))
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

impl TagRepository for RedbMediaRepository {
    type Error = AddTagEdgeError;

    fn init(&self) -> Result<(), Self::Error> {
        crate::media::init_db(self.database()).map_err(AddTagEdgeError::repository)
    }

    fn tag_exists(&self, id: &str) -> Result<bool, Self::Error> {
        let read_txn = self
            .database()
            .begin_read()
            .map_err(AddTagEdgeError::repository)?;
        let table = read_txn
            .open_table(TAG_ADJ_LST)
            .map_err(AddTagEdgeError::repository)?;
        Ok(table
            .get(id)
            .map_err(AddTagEdgeError::repository)?
            .is_some())
    }

    fn add_tag_edge(
        &self,
        media_id: &str,
        tag: &Tag,
        relationship: &str,
    ) -> Result<(), Self::Error> {
        let relationship = normalize_relationship(relationship);
        if relationship.is_empty() {
            return Err(AddTagEdgeError::repository(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "relationship must not be empty",
            )));
        }
        let tag_id = tag.id();
        let write_txn = self
            .database()
            .begin_write()
            .map_err(AddTagEdgeError::repository)?;
        {
            let media_edge = (relationship.clone(), tag_id.to_string());
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

            let tag_edge = (relationship.clone(), media_id.to_string());
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

            let mut tag_name_table = write_txn
                .open_table(TAG)
                .map_err(AddTagEdgeError::repository)?;
            tag_name_table
                .insert(tag_id, tag.name())
                .map_err(AddTagEdgeError::repository)?;

            upsert_relationship_tag_txn(&write_txn, &relationship, tag_id)
                .map_err(AddTagEdgeError::repository)?;
        }
        write_txn.commit().map_err(AddTagEdgeError::repository)?;
        Ok(())
    }

    fn get_tag_adj_list(&self, tag_id: &str) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        get_tag_adj_list(self.database(), tag_id).map_err(AddTagEdgeError::repository)
    }

    fn get_relationships_by_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Self::Error> {
        get_relationships_by_prefix(self.database(), prefix, limit)
            .map_err(AddTagEdgeError::repository)
    }

    fn get_tags_by_relationship(
        &self,
        relationship: &str,
    ) -> Result<Option<Vec<Tag>>, Self::Error> {
        get_tags_by_relationship(self.database(), relationship).map_err(AddTagEdgeError::repository)
    }
}
