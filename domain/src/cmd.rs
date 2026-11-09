use crate::{
    model,
    repository::{CollectionRepository, MediaRepository, SaveMediaError, TagRepository},
};
use std::{
    error::Error as StdError,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
};
use url;

#[derive(Debug)]
pub enum CreateMediaError<E> {
    Io(std::io::Error),
    MediaId(model::MediaIdError),
    Relationship(model::RelationshipError),
    Url(url::ParseError),
    Repository(E),
    Tag(Box<dyn StdError + Send + Sync + 'static>),
}

impl<E: fmt::Display> fmt::Display for CreateMediaError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::MediaId(e) => write!(f, "media id error: {}", e),
            Self::Relationship(e) => write!(f, "relationship error: {}", e),
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
            Self::Relationship(e) => Some(e),
            Self::Url(e) => Some(e),
            Self::Repository(e) => Some(e),
            Self::Tag(e) => Some(e.as_ref()),
        }
    }
}

/// Outcome of attempting to create media from a file.
#[derive(Debug, Clone, PartialEq)]
pub enum CreateMediaOutcome {
    /// A new media record was created.
    Created(model::Media),
    /// Identical content already exists in the repository.
    Duplicate(model::MediaId),
}

/// Creates media from `src_path`, storing the copied file under the default
/// `assets/` directory (relative to the process working directory).
pub fn create_media_from_file<R: MediaRepository>(
    src_path: PathBuf,
    repo: &R,
) -> Result<CreateMediaOutcome, CreateMediaError<R::Error>> {
    create_media_from_file_in(src_path, Path::new("assets"), repo)
}

/// Creates media from `src_path`, storing the copied file under `dest_root`.
///
/// The destination directory is created on demand by the repository.
pub fn create_media_from_file_in<R: MediaRepository>(
    src_path: PathBuf,
    dest_root: &Path,
    repo: &R,
) -> Result<CreateMediaOutcome, CreateMediaError<R::Error>> {
    create_media_from_file_in_with_assignments(src_path, dest_root, repo, None, None)
}

/// Creates media while attaching the selected relationship/tag pairs instead
/// of the default `needs tags` upload pair. For duplicate media, selected
/// pairs are added to existing tags, replacing the default only when it is
/// the duplicate's sole pair.
pub fn create_media_from_file_in_with_tags<R: MediaRepository>(
    src_path: PathBuf,
    dest_root: &Path,
    tags: &[(String, String)],
    repo: &R,
) -> Result<CreateMediaOutcome, CreateMediaError<R::Error>> {
    create_media_from_file_in_with_tags_and_title(src_path, dest_root, tags, None, repo)
}

pub fn create_media_from_file_in_with_tags_and_title<R: MediaRepository>(
    src_path: PathBuf,
    dest_root: &Path,
    tags: &[(String, String)],
    title: Option<String>,
    repo: &R,
) -> Result<CreateMediaOutcome, CreateMediaError<R::Error>> {
    let mut assignments = Vec::with_capacity(tags.len());
    for (relationship, tag_name) in tags {
        let relationship =
            model::new_relationship(relationship).map_err(CreateMediaError::Relationship)?;
        let tag = model::Tag::new(tag_name).map_err(|error| {
            CreateMediaError::Tag(Box::new(error) as Box<dyn StdError + Send + Sync>)
        })?;
        if !assignments.iter().any(
            |(existing_relationship, existing_tag): &(String, model::Tag)| {
                existing_relationship == &relationship && existing_tag == &tag
            },
        ) {
            assignments.push((relationship, tag));
        }
    }
    create_media_from_file_in_with_assignments(src_path, dest_root, repo, Some(&assignments), title)
}

fn create_media_from_file_in_with_assignments<R: MediaRepository>(
    src_path: PathBuf,
    dest_root: &Path,
    repo: &R,
    tags: Option<&[(String, model::Tag)]>,
    title: Option<String>,
) -> Result<CreateMediaOutcome, CreateMediaError<R::Error>> {
    let mut file = fs::File::open(&src_path).map_err(CreateMediaError::Io)?;
    let file_size = file.metadata().map_err(CreateMediaError::Io)?.len();
    let mime_type = infer::get_from_path(&src_path)
        .map_err(CreateMediaError::Io)?
        .map(|kind| kind.mime_type().to_string());
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
        if let Some(tags) = tags.filter(|tags| !tags.is_empty()) {
            repo.apply_upload_tags_to_duplicate(&id, tags)
                .map_err(CreateMediaError::Repository)?;
        }
        return Ok(CreateMediaOutcome::Duplicate(id));
    }
    let file_ext = src_path.extension().unwrap_or(std::ffi::OsStr::new(""));
    let dest_path = dest_root.join(id.as_str()).with_extension(file_ext);
    let url = url::Url::parse(&format!("file:///{}", dest_path.display()))
        .map_err(CreateMediaError::Url)?;
    let media = model::Media::new(
        id,
        url,
        Some(model::MediaOpts {
            title,
            mime_type,
            file_size: Some(file_size),
            view_count: None,
        }),
    );
    let save_result = match tags.filter(|tags| !tags.is_empty()) {
        Some(tags) => repo.save_media_with_tags(&media, &src_path, &dest_path, tags),
        None => repo.save_media(&media, &src_path, &dest_path),
    };
    save_result.map_err(|e| match e {
        SaveMediaError::Io(io) => CreateMediaError::Io(io),
        SaveMediaError::Repository(r) => CreateMediaError::Repository(r),
        SaveMediaError::Tag(t) => CreateMediaError::Tag(t),
    })?;

    Ok(CreateMediaOutcome::Created(media))
}

#[derive(Debug)]
pub enum AddTagError<E> {
    EmptyMediaId,
    EmptyRelationship,
    Tag(model::TagError),
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for AddTagError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMediaId => write!(f, "media id must not be empty"),
            Self::EmptyRelationship => write!(f, "relationship must not be empty"),
            Self::Tag(e) => write!(f, "tag error: {}", e),
            Self::Repository(e) => write!(f, "repository error: {}", e),
        }
    }
}

impl<E: StdError + 'static> StdError for AddTagError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyMediaId => None,
            Self::EmptyRelationship => None,
            Self::Tag(e) => Some(e),
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn add_tag_to_media<R: TagRepository>(
    media_id: &str,
    tag_name: &str,
    relationship: &str,
    repo: &R,
) -> Result<model::Tag, AddTagError<R::Error>> {
    if media_id.is_empty() {
        return Err(AddTagError::EmptyMediaId);
    }
    if relationship.is_empty() {
        return Err(AddTagError::EmptyRelationship);
    }
    let tag = model::Tag::new(tag_name).map_err(AddTagError::Tag)?;
    repo.add_tag_edge(media_id, &tag, relationship)
        .map_err(AddTagError::Repository)?;
    Ok(tag)
}

pub fn add_tag_to_media_items<R: TagRepository>(
    media_ids: &[String],
    tag_name: &str,
    relationship: &str,
    repo: &R,
) -> Result<usize, AddTagError<R::Error>> {
    if relationship.trim().is_empty() {
        return Err(AddTagError::EmptyRelationship);
    }
    let tag = model::Tag::new(tag_name).map_err(AddTagError::Tag)?;
    let relationship = model::normalize_relationship(relationship);
    if relationship.is_empty() {
        return Err(AddTagError::EmptyRelationship);
    }
    let mut count = 0;
    for media_id in media_ids {
        if media_id.is_empty() {
            return Err(AddTagError::EmptyMediaId);
        }
        repo.add_tag_edge(media_id, &tag, &relationship)
            .map_err(AddTagError::Repository)?;
        count += 1;
    }
    Ok(count)
}

#[derive(Debug)]
pub enum RemoveTagError<E> {
    EmptyMediaId,
    EmptyTagId,
    EmptyRelationship,
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for RemoveTagError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMediaId => write!(f, "media id must not be empty"),
            Self::EmptyTagId => write!(f, "tag id must not be empty"),
            Self::EmptyRelationship => write!(f, "relationship must not be empty"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for RemoveTagError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyMediaId | Self::EmptyTagId | Self::EmptyRelationship => None,
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn remove_tag_from_media<R: TagRepository>(
    media_id: &str,
    tag_id: &str,
    relationship: &str,
    repo: &R,
) -> Result<(), RemoveTagError<R::Error>> {
    if media_id.is_empty() {
        return Err(RemoveTagError::EmptyMediaId);
    }
    if tag_id.is_empty() {
        return Err(RemoveTagError::EmptyTagId);
    }
    let relationship = model::normalize_relationship(relationship);
    if relationship.is_empty() {
        return Err(RemoveTagError::EmptyRelationship);
    }
    repo.remove_tag_edge(media_id, tag_id, &relationship)
        .map_err(RemoveTagError::Repository)
}

#[derive(Debug)]
pub enum MergeRelationshipError<E> {
    EmptySourceRelationship,
    EmptyDestinationRelationship,
    SameRelationship,
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for MergeRelationshipError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceRelationship => write!(f, "source relationship must not be empty"),
            Self::EmptyDestinationRelationship => {
                write!(f, "destination relationship must not be empty")
            }
            Self::SameRelationship => {
                write!(f, "source and destination relationships must differ")
            }
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for MergeRelationshipError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptySourceRelationship
            | Self::EmptyDestinationRelationship
            | Self::SameRelationship => None,
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn merge_relationship<R: TagRepository>(
    source_relationship: &str,
    dest_relationship: &str,
    repo: &R,
) -> Result<(), MergeRelationshipError<R::Error>> {
    let source = model::normalize_relationship(source_relationship);
    if source.is_empty() {
        return Err(MergeRelationshipError::EmptySourceRelationship);
    }
    let dest = model::normalize_relationship(dest_relationship);
    if dest.is_empty() {
        return Err(MergeRelationshipError::EmptyDestinationRelationship);
    }
    if source == dest {
        return Err(MergeRelationshipError::SameRelationship);
    }
    repo.merge_relationship(&source, &dest)
        .map_err(MergeRelationshipError::Repository)
}

#[derive(Debug)]
pub enum AliasTagError<E> {
    EmptyTagId,
    SameTag,
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for AliasTagError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTagId => write!(f, "tag id must not be empty"),
            Self::SameTag => write!(f, "tags must differ"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for AliasTagError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyTagId | Self::SameTag => None,
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn alias_tags<R: TagRepository>(
    tag_a: &str,
    tag_b: &str,
    repo: &R,
) -> Result<(), AliasTagError<R::Error>> {
    if tag_a.trim().is_empty() || tag_b.trim().is_empty() {
        return Err(AliasTagError::EmptyTagId);
    }
    let tag_a = model::Tag::new(tag_a).map_err(|_| AliasTagError::EmptyTagId)?;
    let tag_b = model::Tag::new(tag_b).map_err(|_| AliasTagError::EmptyTagId)?;
    if tag_a == tag_b {
        return Err(AliasTagError::SameTag);
    }
    repo.upsert_tag(&tag_a).map_err(AliasTagError::Repository)?;
    repo.upsert_tag(&tag_b).map_err(AliasTagError::Repository)?;
    repo.alias_tags(tag_a.id(), tag_b.id())
        .map_err(AliasTagError::Repository)
}

pub fn unalias_tags<R: TagRepository>(
    tag_a: &str,
    tag_b: &str,
    repo: &R,
) -> Result<(), AliasTagError<R::Error>> {
    if tag_a.trim().is_empty() || tag_b.trim().is_empty() {
        return Err(AliasTagError::EmptyTagId);
    }
    let tag_a = model::Tag::new(tag_a).map_err(|_| AliasTagError::EmptyTagId)?;
    let tag_b = model::Tag::new(tag_b).map_err(|_| AliasTagError::EmptyTagId)?;
    if tag_a == tag_b {
        return Err(AliasTagError::SameTag);
    }
    repo.unalias_tags(tag_a.id(), tag_b.id())
        .map_err(AliasTagError::Repository)
}

#[derive(Debug)]
pub enum SaveCollectionError<E> {
    EmptyName,
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for SaveCollectionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "collection name must not be empty"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for SaveCollectionError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyName => None,
            Self::Repository(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum DeleteCollectionError<E> {
    EmptyName,
    Repository(E),
}

impl<E: fmt::Display> fmt::Display for DeleteCollectionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "collection name must not be empty"),
            Self::Repository(e) => write!(f, "repository error: {e}"),
        }
    }
}

impl<E: StdError + 'static> StdError for DeleteCollectionError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::EmptyName => None,
            Self::Repository(e) => Some(e),
        }
    }
}

pub fn save_collection<R: CollectionRepository>(
    collection: &model::Collection,
    repo: &R,
) -> Result<(), SaveCollectionError<R::Error>> {
    let name = model::normalize_collection_name(&collection.name);
    if name.is_empty() {
        return Err(SaveCollectionError::EmptyName);
    }
    let mut collection = collection.clone();
    collection.name = name;
    repo.save_collection(&collection)
        .map_err(SaveCollectionError::Repository)
}

pub fn delete_collection<R: CollectionRepository>(
    name: &str,
    repo: &R,
) -> Result<bool, DeleteCollectionError<R::Error>> {
    let name = model::normalize_collection_name(name);
    if name.is_empty() {
        return Err(DeleteCollectionError::EmptyName);
    }
    repo.delete_collection(&name)
        .map_err(DeleteCollectionError::Repository)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{MediaRepository, SaveMediaError, TagRepository};
    use std::collections::HashSet;

    impl CreateMediaOutcome {
        /// Test helper: unwraps a `Created` outcome, panicking on duplicate.
        fn expect_created(self, msg: &str) -> model::Media {
            match self {
                Self::Created(media) => media,
                Self::Duplicate(id) => panic!("{msg}: unexpected duplicate for {}", id.as_str()),
            }
        }
    }

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

        fn save_media_with_tags(
            &self,
            _media: &model::Media,
            _src_path: &std::path::Path,
            _dest_path: &std::path::Path,
            _tags: &[(String, model::Tag)],
        ) -> Result<(), SaveMediaError<Self::Error>> {
            self.save_media(_media, _src_path, _dest_path)
        }

        fn apply_upload_tags_to_duplicate(
            &self,
            _media_id: &model::MediaId,
            _tags: &[(String, model::Tag)],
        ) -> Result<model::ApplyUploadTagsOutcome, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(model::ApplyUploadTagsOutcome::Applied)
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

        fn get_media_adj_list(
            &self,
            _id: &model::MediaId,
        ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
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
            _tag: &model::Tag,
            _relationship: &str,
        ) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }

        fn remove_tag_edge(
            &self,
            _media_id: &str,
            _tag_id: &str,
            _relationship: &str,
        ) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }

        fn get_tag_adj_list(
            &self,
            _tag_id: &str,
        ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }

        fn get_relationships_by_prefix(
            &self,
            _prefix: &str,
            _limit: Option<usize>,
        ) -> Result<Option<Vec<String>>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }

        fn get_tags_by_relationship(
            &self,
            _relationship: &str,
        ) -> Result<Option<Vec<model::Tag>>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }

        fn merge_relationship(
            &self,
            _source_relationship: &str,
            _dest_relationship: &str,
        ) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }
    }

    impl CollectionRepository for StubRepo {
        type Error = StubError;

        fn init(&self) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }

        fn save_collection(&self, _collection: &model::Collection) -> Result<(), Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(())
            }
        }

        fn get_collection(&self, _name: &str) -> Result<Option<model::Collection>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }

        fn list_collections(&self) -> Result<Vec<model::Collection>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(Vec::new())
            }
        }

        fn get_collections_by_prefix(
            &self,
            _prefix: &str,
            _limit: Option<usize>,
        ) -> Result<Option<Vec<model::Collection>>, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(None)
            }
        }

        fn delete_collection(&self, _name: &str) -> Result<bool, Self::Error> {
            if self.error.is_some() {
                Err(StubError)
            } else {
                Ok(false)
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
            .expect_created("new media should be created");

        let hash = blake3_hex(contents);
        assert_eq!(media.id.as_str(), hash);
        assert_eq!(media.url.as_str(), format!("file:///assets/{hash}.jpg"));
        assert_eq!(media.title, None);
        assert_eq!(media.mime_type, None);
        assert_eq!(media.file_size, Some(contents.len() as u64));
        assert_eq!(media.view_count, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_create_media_from_file_detects_mime_type() {
        let contents = b"\x89PNG\r\n\x1a\n";
        let path = temp_file("detect_mime_type", "bin", contents);
        let repo = StubRepo::new(&[]);

        let media = create_media_from_file(path.clone(), &repo)
            .unwrap()
            .expect_created("new media should be created");

        assert_eq!(media.mime_type.as_deref(), Some("image/png"));
        assert_eq!(media.file_size, Some(contents.len() as u64));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_create_media_from_file_duplicate_media() {
        let contents = b"recondo test media";
        let path = temp_file("duplicate_media", "bin", contents);
        let repo = StubRepo::new(&[blake3_hex(contents).as_str()]);

        let result = create_media_from_file(path.clone(), &repo);

        assert!(
            matches!(
                &result,
                Ok(CreateMediaOutcome::Duplicate(id)) if id.as_str() == blake3_hex(contents)
            ),
            "expected duplicate outcome, got {result:?}"
        );

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
    fn test_add_tag_to_media_empty_relationship() {
        let repo = StubRepo::new(&[]);
        let result = add_tag_to_media("media1", "landscape", "", &repo);
        assert!(matches!(result, Err(AddTagError::EmptyRelationship)));
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

    #[test]
    fn test_merge_relationship_success() {
        let repo = StubRepo::new(&[]);
        assert!(merge_relationship("Author", "Creator", &repo).is_ok());
    }

    #[test]
    fn test_merge_relationship_empty_source() {
        let repo = StubRepo::new(&[]);
        let result = merge_relationship("   ", "creator", &repo);
        assert!(matches!(
            result,
            Err(MergeRelationshipError::EmptySourceRelationship)
        ));
    }

    #[test]
    fn test_merge_relationship_empty_destination() {
        let repo = StubRepo::new(&[]);
        let result = merge_relationship("author", "   ", &repo);
        assert!(matches!(
            result,
            Err(MergeRelationshipError::EmptyDestinationRelationship)
        ));
    }

    #[test]
    fn test_merge_relationship_same_relationship() {
        let repo = StubRepo::new(&[]);
        let result = merge_relationship("Author", "  author  ", &repo);
        assert!(matches!(
            result,
            Err(MergeRelationshipError::SameRelationship)
        ));
    }

    #[test]
    fn test_merge_relationship_repository_error() {
        let repo = StubRepo::failing();
        let result = merge_relationship("author", "creator", &repo);
        assert!(matches!(result, Err(MergeRelationshipError::Repository(_))));
    }

    #[test]
    fn test_alias_tags_empty() {
        let repo = StubRepo::new(&[]);
        let result = alias_tags("", "tag2", &repo);
        assert!(matches!(result, Err(AliasTagError::EmptyTagId)));
    }

    #[test]
    fn test_alias_tags_same() {
        let repo = StubRepo::new(&[]);
        let result = alias_tags("tag1", "tag1", &repo);
        assert!(matches!(result, Err(AliasTagError::SameTag)));
    }
}

pub fn recycle_media<R: crate::repository::MediaRepository>(
    id: &crate::model::MediaId,
    repo: &R,
) -> Result<Option<crate::model::RecycledMedia>, R::Error> {
    repo.recycle_media(id)
}

pub fn restore_media<R: crate::repository::MediaRepository>(
    id: &crate::model::MediaId,
    repo: &R,
) -> Result<Option<crate::model::RecycledMedia>, R::Error> {
    repo.restore_recycled_media(id)
}

pub fn permanently_delete_media<R: crate::repository::MediaRepository>(
    id: &crate::model::MediaId,
    assets_dir: &std::path::Path,
    repo: &R,
) -> Result<bool, crate::repository::DeleteMediaError<R::Error>> {
    repo.delete_recycled_media(id, assets_dir)
}

pub fn empty_recycle_bin<R: crate::repository::MediaRepository>(
    assets_dir: &std::path::Path,
    repo: &R,
) -> Result<usize, crate::repository::DeleteMediaError<R::Error>> {
    repo.empty_recycle_bin(assets_dir)
}

pub fn remove_tag_from_media_items<R, E>(
    media_ids: &[String],
    tag_id: &str,
    repo: &R,
) -> Result<usize, RemoveTagError<E>>
where
    R: TagRepository<Error = E> + MediaRepository<Error = E>,
    E: std::fmt::Display + std::error::Error + 'static,
{
    if tag_id.is_empty() {
        return Err(RemoveTagError::EmptyTagId);
    }
    let mut count = 0;
    for media_id in media_ids {
        if media_id.is_empty() {
            return Err(RemoveTagError::EmptyMediaId);
        }
        let media_id_parsed = model::MediaId::from_hash(media_id.clone())
            .map_err(|_| RemoveTagError::EmptyMediaId)?;
        let edges = repo
            .get_media_adj_list(&media_id_parsed)
            .map_err(RemoveTagError::Repository)?
            .unwrap_or_default();
        for (relationship, edge_tag_id) in edges {
            if edge_tag_id == tag_id {
                repo.remove_tag_edge(media_id, tag_id, &relationship)
                    .map_err(RemoveTagError::Repository)?;
                count += 1;
            }
        }
    }
    Ok(count)
}
