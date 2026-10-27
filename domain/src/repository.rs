use crate::model::{Media, MediaId};
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
    fn save_media_record(&self, media: &Media) -> Result<(), Self::Error>;
    fn get_media(&self, id: &MediaId) -> Result<Option<Media>, Self::Error>;
    fn get_media_adj_list(
        &self,
        id: &MediaId,
    ) -> Result<Option<Vec<(String, String)>>, Self::Error>;
}

pub trait TagRepository {
    type Error;
    fn init(&self) -> Result<(), Self::Error>;
    fn tag_exists(&self, id: &str) -> Result<bool, Self::Error>;
    fn add_tag_edge(
        &self,
        media_id: &str,
        tag_id: &str,
        label: &str,
    ) -> Result<(), Self::Error>;
    fn get_tag_adj_list(
        &self,
        tag_id: &str,
    ) -> Result<Option<Vec<(String, String)>>, Self::Error>;
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

