use crate::{
    model,
    repository::{MediaRepository, SaveMediaError, TagRepository},
};
use std::{error::Error as StdError, fmt, fs, io::Read, path::PathBuf};
use url;

#[derive(Debug)]
pub enum CreateMediaError<E> {
    Io(std::io::Error),
    MediaId(model::MediaIdError),
    Url(url::ParseError),
    Repository(E),
    Tag(Box<dyn StdError + Send + Sync + 'static>),
}

impl<E: fmt::Display> fmt::Display for CreateMediaError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::MediaId(e) => write!(f, "media id error: {}", e),
            Self::Url(e) => write!(f, "url parse error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
            Self::Tag(e) => write!(f, "tag error: {}", e),
        }
    }
}

impl<E: StdError + 'static> StdError for CreateMediaError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::MediaId(e) => Some(e),
            Self::Url(e) => Some(e),
            Self::Repository(e) => Some(e),
            Self::Tag(e) => Some(e.as_ref()),
        }
    }
}

pub fn create_media_from_file<R: MediaRepository>(
    src_path: PathBuf,
    repo: &R,
) -> Result<Option<model::Media>, CreateMediaError<R::Error>> {
    let mut file = fs::File::open(&src_path).map_err(CreateMediaError::Io)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(CreateMediaError::Io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize().to_hex().to_string();
    let id = model::MediaId::from_hash(hash).map_err(CreateMediaError::MediaId)?;
    if repo
        .media_exists(&id)
        .map_err(CreateMediaError::Repository)?
    {
        return Ok(None);
    }
    let file_ext = src_path.extension().unwrap_or(std::ffi::OsStr::new(""));
    let dest_path = PathBuf::from("assets/")
        .join(id.as_str())
        .with_extension(file_ext);
    let url = url::Url::parse(&format!("file:///{}", dest_path.display()))
        .map_err(CreateMediaError::Url)?;
    let media = model::Media::new(id, url, None);
    repo.save_media(&media, &src_path, &dest_path)
        .map_err(|e| match e {
            SaveMediaError::Io(io) => CreateMediaError::Io(io),
            SaveMediaError::Repository(r) => CreateMediaError::Repository(r),
            SaveMediaError::Tag(t) => CreateMediaError::Tag(t),
        })?;
    Ok(Some(media))
}

#[derive(Debug)]
pub enum AddTagError<E> {
    EmptyMediaId,
    EmptyLabel,
    Tag(model::TagError),
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for AddTagError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMediaId => write!(f, "media id must not be empty"),
            Self::EmptyLabel => write!(f, "label must not be empty"),
            Self::Tag(e) => write!(f, "tag error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
        }
    }
}

impl<E: StdError + 'static> StdError for AddTagError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyMediaId => None,
            Self::EmptyLabel => None,
            Self::Tag(e) => Some(e),
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn add_tag_to_media<R: TagRepository>(
    media_id: &str,
    tag_name: &str,
    label: &str,
    repo: &R,
) -> Result<model::Tag, AddTagError<R::Error>> {
    if media_id.is_empty() {
        return Err(AddTagError::EmptyMediaId);
    }
    if label.is_empty() {
        return Err(AddTagError::EmptyLabel);
    }
    let tag = model::Tag::new(tag_name).map_err(AddTagError::Tag)?;
    repo.add_tag_edge(media_id, tag.id(), label)
        .map_err(AddTagError::Repository)?;
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{MediaRepository, SaveMediaError, TagRepository};
    use std::collections::HashSet;

    #[derive(Debug)]
    struct StubError;

    impl fmt::Display for StubError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "stub repository failure")
        }
    }

    impl StdError for StubError {}

    /// Stub repository whose `media_exists` answers from a fixed id set.
    /// With `error` set, every repository call fails instead.
    struct StubRepo {
        existing: HashSet<String>,
        error: Option<StubError>,
    }

    impl StubRepo {
        fn new(existing: &[&str]) -> Self {
            Self {
                existing: existing.iter().map(|s| s.to_string()).collect(),
                error: None,
            }
        }

        fn failing() -> Self {
            Self {
                existing: HashSet::new(),
                error: Some(StubError),
            }
        }
    }

    impl MediaRepository for StubRepo {
        type Error = StubError;

        fn init(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn media_exists(&self, id: &model::MediaId) -> Result<bool, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(self.existing.contains(id.as_str()))
            }
        }

        fn save_media(
            &self,
            _media: &model::Media,
            _src_path: &std::path::Path,
            _dest_path: &std::path::Path,
        ) -> Result<(), SaveMediaError<Self::Error>> {
            if self.error.is_some() {
                Err(SaveMediaError::Repository(StubError))
            } else {
                Ok(())
            }
        }

        fn save_media_record(&self, _media: &model::Media) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }

        fn get_media(&self, _id: &model::MediaId) -> Result<Option<model::Media>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }
    }

    impl TagRepository for StubRepo {
        type Error = StubError;

        fn init(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn tag_exists(&self, _id: &str) -> Result<bool, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(false)
            }
        }

        fn add_tag_edge(
            &self,
            _media_id: &str,
            _tag_id: &str,
            _label: &str,
        ) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }
    }

    /// Creates a uniquely named temp file for the given contents.
    fn temp_file(test: &str, ext: &str, contents: &[u8]) -> PathBuf {
        let name = format!("recondo_cmd_test_{}_{}.{}", test, std::process::id(), ext);
        let path = std::env::temp_dir().join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn blake3_hex(contents: &[u8]) -> String {
        hex::encode(blake3::hash(contents).as_bytes())
    }

    #[test]
    fn test_create_media_from_file_new_media() {
        let contents = b"recondo test media";
        let path = temp_file("new_media", "jpg", contents);
        let repo = StubRepo::new(&[]);

        let media = create_media_from_file(path.clone(), &repo)
            .unwrap()
            .expect("new media should be created");

        let hash = blake3_hex(contents);
        assert_eq!(media.id.as_str(), hash);
        assert_eq!(media.url.as_str(), format!("file:///assets/{hash}.jpg"));
        assert_eq!(media.title, None);
        assert_eq!(media.mime_type, None);
        assert_eq!(media.file_size, None);
        assert_eq!(media.view_count, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_create_media_from_file_duplicate_media() {
        let contents = b"recondo test media";
        let path = temp_file("duplicate_media", "bin", contents);
        let repo = StubRepo::new(&[blake3_hex(contents).as_str()]);

        let result = create_media_from_file(path.clone(), &repo);

        assert!(matches!(result, Ok(None)));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_create_media_from_file_missing_file() {
        let path =
            std::env::temp_dir().join(format!("recondo_cmd_test_missing_{}", std::process::id()));
        let _ = fs::remove_file(&path); // ensure the file does not exist
        let repo = StubRepo::new(&[]);

        let result = create_media_from_file(path, &repo);

        assert!(matches!(result, Err(CreateMediaError::Io(_))));
    }

    #[test]
    fn test_create_media_from_file_repository_error() {
        let contents = b"recondo test media";
        let path = temp_file("repository_error", "bin", contents);
        let repo = StubRepo::failing();

        let result = create_media_from_file(path.clone(), &repo);

        assert!(matches!(result, Err(CreateMediaError::Repository(_))));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_add_tag_to_media_success() {
        let repo = StubRepo::new(&[]);
        let tag = add_tag_to_media("media1", "landscape", "liked", &repo).unwrap();
        assert_eq!(tag.name(), "landscape");
    }

    #[test]
    fn test_add_tag_to_media_empty_media_id() {
        let repo = StubRepo::new(&[]);
        let result = add_tag_to_media("", "landscape", "liked", &repo);
        assert!(matches!(result, Err(AddTagError::EmptyMediaId)));
    }

    #[test]
    fn test_add_tag_to_media_empty_label() {
        let repo = StubRepo::new(&[]);
        let result = add_tag_to_media("media1", "landscape", "", &repo);
        assert!(matches!(result, Err(AddTagError::EmptyLabel)));
    }

    #[test]
    fn test_add_tag_to_media_empty_tag_name() {
        let repo = StubRepo::new(&[]);
        let result = add_tag_to_media("media1", "   ", "liked", &repo);
        assert!(matches!(result, Err(AddTagError::Tag(_))));
    }

    #[test]
    fn test_add_tag_to_media_repository_error() {
        let repo = StubRepo::failing();
        let result = add_tag_to_media("media1", "landscape", "liked", &repo);
        assert!(matches!(result, Err(AddTagError::Repository(_))));
    }
}
