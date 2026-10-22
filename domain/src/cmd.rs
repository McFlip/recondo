use crate::model;

pub fn create_media_from_file() -> model::Media {
    model::Media::new("fubar".into(), "assets/fubar.jpg".into(), None)
}