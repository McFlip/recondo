use crate::model;
use url;

pub fn create_media_from_file() -> model::Media {
    let id = model::MediaId::from_hash("fubar").expect("valid media id");
    let url = url::Url::parse("file:///assets/fubar.jpg").expect("valid url");
    model::Media::new(id, url, None)
}