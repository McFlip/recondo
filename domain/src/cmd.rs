use crate::model;
use std::{ffi::OsStr, path::PathBuf};
use url;

pub fn create_media_from_file(src_path: PathBuf) -> model::Media {
    let id = model::MediaId::from_hash("fubar").expect("valid media id");
    // let file_ext = ".jpg".to_string();
    let file_ext = src_path.extension().unwrap_or(OsStr::new(""));
    let url = PathBuf::from("assets/")
        .join(id.as_str())
        .set_extension(file_ext);
    let url = url::Url::parse(&format!("file:///{}", url)).expect("valid url");
    model::Media::new(id, url, None)
}
