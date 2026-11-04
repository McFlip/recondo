use crate::model::{Media, MediaId, RecycledMedia, Tag};
use std::fmt;
use std::path::Path;

pub trait MediaRepository {
    type Error;
    fn init(&self) -> Result<(), Self::Error>;
    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error>;
    fn save_media(
        &self,
        media: &Media,
        src_path: &Path,
        dest_path: &Path,
    ) -> Result<(), SaveMediaError<Self::Error>>;
    fn save_media_with_tags(
        &self,
        media: &Media,
        src_path: &Path,
        dest_path: &Path,
        tags: &[(String, Tag)],
    ) -> Result<(), SaveMediaError<Self::Error>>;
    fn apply_upload_tags_to_duplicate(
        &self,
        media_id: &MediaId,
        tags: &[(String, Tag)],
    ) -> Result<crate::model::ApplyUploadTagsOutcome, Self::Error>;
    fn save_media_record(&self, media: &Media) -> Result<(), Self::Error>;
    fn get_media(&self, id: &MediaId) -> Result<Option<Media>, Self::Error>;
    fn get_media_adj_list(
        &self,
        id: &MediaId,
    ) -> Result<Option<Vec<(String, String)>>, Self::Error>;
    fn recycle_media(&self, _id: &MediaId) -> Result<Option<RecycledMedia>, Self::Error> { Ok(None) }
    fn restore_recycled_media(&self, _id: &MediaId) -> Result<Option<RecycledMedia>, Self::Error> { Ok(None) }
    fn get_recycled_media(&self, _id: &MediaId) -> Result<Option<RecycledMedia>, Self::Error> { Ok(None) }
    fn list_recycled_media(&self) -> Result<Vec<RecycledMedia>, Self::Error> { Ok(Vec::new()) }
    fn delete_recycled_media(&self, _id: &MediaId, _assets_dir: &Path) -> Result<bool, DeleteMediaError<Self::Error>> { Ok(false) }
    fn empty_recycle_bin(&self, _assets_dir: &Path) -> Result<usize, DeleteMediaError<Self::Error>> { Ok(0) }
}

pub trait TagRepository {
    type Error;
    fn init(&self) -> Result<(), Self::Error>;
    fn tag_exists(&self, id: &str) -> Result<bool, Self::Error>;
    fn add_tag_edge(
        &self,
        media_id: &str,
        tag: &Tag,
        relationship: &str,
    ) -> Result<(), Self::Error>;
    fn remove_tag_edge(
        &self,
        media_id: &str,
        tag_id: &str,
        relationship: &str,
    ) -> Result<(), Self::Error>;
    fn get_tag_adj_list(&self, tag_id: &str) -> Result<Option<Vec<(String, String)>>, Self::Error>;
    fn get_relationships_by_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Self::Error>;
    fn get_tags_by_relationship(&self, relationship: &str)
    -> Result<Option<Vec<Tag>>, Self::Error>;
    fn merge_relationship(
        &self,
        source_relationship: &str,
        dest_relationship: &str,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum SaveMediaError<E> {
    Io(std::io::Error),
    Repository(E),
    Tag(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<E: std::fmt::Display> std::fmt::Display for SaveMediaError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
            Self::Tag(e) => write!(f, "tag error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SaveMediaError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Repository(e) => Some(e),
            Self::Tag(e) => Some(e.as_ref()),
        }
    }
}

#[derive(Debug)]
pub enum DeleteMediaError<E> {
    Io(std::io::Error),
    Repository(E),
}

impl<E: std::fmt::Display> std::fmt::Display for DeleteMediaError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DeleteMediaError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Repository(e) => Some(e),
        }
    }
}
