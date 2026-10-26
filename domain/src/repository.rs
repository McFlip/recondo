use crate::model::MediaId;
use std::path::Path;

pub trait MediaRepository {
    type Error;
    fn init(&self) -> Result<(), Self::Error>;
    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error>;
    fn save_media(
        &self,
        src_path: &Path,
        dest_path: &Path,
    ) -> Result<(), SaveMediaError<Self::Error>>;
}

#[derive(Debug)]
pub enum SaveMediaError<E> {
    Io(std::io::Error),
    Repository(E),
}

impl<E: std::fmt::Display> std::fmt::Display for SaveMediaError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SaveMediaError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Repository(e) => Some(e),
        }
    }
}