use crate::{model, repository::MediaRepository};
use std::{ffi::OsStr, path::PathBuf};
use url;

pub fn create_media_from_file<R: MediaRepository>(
    src_path: PathBuf,
    repo: &R,
) -> Result<Option<model::Media>, R::Error> {
    let id = model::MediaId::from_hash("fubar").expect("valid media id");
    if repo.media_exists(&id)? {
        return Ok(None);
    }
    let file_ext = src_path.extension().unwrap_or(OsStr::new(""));
    let url = PathBuf::from("assets/")
        .join(id.as_str())
        .with_extension(file_ext);
    let url = url::Url::parse(&format!("file:///{}", url.display())).expect("valid url");
    Ok(Some(model::Media::new(id, url, None)))
}
