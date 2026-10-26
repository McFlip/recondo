use crate::model::MediaId;

pub trait MediaRepository {
    type Error;
    fn init(&self) -> Result<(), Self::Error>;
    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error>;
}
