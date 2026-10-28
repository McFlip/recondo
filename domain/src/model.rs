use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaId(String);

impl MediaId {
    pub fn from_hash(s: impl Into<String>) -> Result<Self, MediaIdError> {
        let s = s.into();
        if s.is_empty() {
            return Err(MediaIdError::Empty);
        }
        // Assume it's a blake3 hash hex string as per requirements
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaIdError {
    Empty,
}

impl fmt::Display for MediaIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "media id must not be empty"),
        }
    }
}

impl Error for MediaIdError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    pub id: MediaId,
    pub url: Url,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
    pub view_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaOpts {
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
    pub view_count: Option<u64>,
}

impl Media {
    pub fn new(id: MediaId, url: Url, opts: Option<MediaOpts>) -> Self {
        let opts = opts.as_ref();
        Self {
            id,
            url,
            title: opts.and_then(|o| o.title.clone()),
            mime_type: opts.and_then(|o| o.mime_type.clone()),
            file_size: opts.and_then(|o| o.file_size),
            view_count: opts.and_then(|o| o.view_count).unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagError {
    EmptyName,
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "tag name must not be empty"),
        }
    }
}

impl Error for TagError {}

impl Tag {
    /// Creates a tag, normalizing its name to trimmed, lowercase NFC.
    ///
    /// The tag's ID is the BLAKE3 hash of the normalized name. Because the name
    /// is immutable, changing a tag means creating a new tag and updating any
    /// references to it.
    pub fn new(name: impl AsRef<str>) -> Result<Self, TagError> {
        let name: String = name.as_ref().trim().to_lowercase().nfc().collect();
        if name.is_empty() {
            return Err(TagError::EmptyName);
        }
        let id = hex::encode(blake3::hash(name.as_bytes()).as_bytes());
        Ok(Self { id, name })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn from_parts(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }

    /// Checks whether this tag already exists in the repository's tag adjacency list.
    pub fn check<R: crate::repository::TagRepository>(
        &self,
        repo: &R,
    ) -> Result<bool, R::Error> {
        repo.tag_exists(self.id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipError {
    Empty,
}

impl fmt::Display for RelationshipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "relationship must not be empty"),
        }
    }
}

impl Error for RelationshipError {}

pub fn normalize_relationship(s: impl AsRef<str>) -> String {
    s.as_ref().trim().to_lowercase().nfc().collect()
}

pub fn new_relationship(s: impl AsRef<str>) -> Result<String, RelationshipError> {
    let rel = normalize_relationship(s);
    if rel.is_empty() {
        Err(RelationshipError::Empty)
    } else {
        Ok(rel)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    label: String,
    media: String,
    tag: String,
}

impl Edge {
    pub fn new(label: String, media: String, tag: String) -> Result<Self, EdgeError> {
        let label = normalize_relationship(label);
        if label.is_empty() {
            return Err(EdgeError::EmptyLabel);
        }
        if media.is_empty() {
            return Err(EdgeError::EmptyMedia);
        }
        if tag.is_empty() {
            return Err(EdgeError::EmptyTag);
        }
        Ok(Self { label, media, tag })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeError {
    EmptyLabel,
    EmptyMedia,
    EmptyTag,
}

impl fmt::Display for EdgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLabel => write!(f, "label must not be empty"),
            Self::EmptyMedia => write!(f, "media ID must not be empty"),
            Self::EmptyTag => write!(f, "tag ID must not be empty"),
        }
    }
}

impl Error for EdgeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_new() {
        let tag = Tag::new("landscape").unwrap();
        assert_eq!(tag.name(), "landscape");
        // blake3("landscape") = 97154c667a3d843bcbd739947cfb383380356eafadbb92342984fe6c0956cb18
        assert_eq!(
            tag.id(),
            "97154c667a3d843bcbd739947cfb383380356eafadbb92342984fe6c0956cb18"
        );
    }

    #[test]
    fn test_tag_new_normalizes_name() {
        let tag = Tag::new("  LaNdScApE  ").unwrap();
        assert_eq!(tag.name(), "landscape");
        assert_eq!(tag, Tag::new("landscape").unwrap());
    }

    #[test]
    fn test_tag_new_normalizes_unicode() {
        let composed = Tag::new("caf\u{e9}").unwrap();
        let decomposed = Tag::new("cafe\u{301}").unwrap();
        assert_eq!(composed, decomposed);
    }

    #[test]
    fn test_tag_new_rejects_empty_name() {
        assert_eq!(Tag::new(""), Err(TagError::EmptyName));
        assert_eq!(Tag::new("   \t\n"), Err(TagError::EmptyName));
    }

    #[test]
    fn test_edge_new_success() {
        let edge = Edge::new(
            "liked".to_string(),
            "media_id_1".to_string(),
            "tag_id_1".to_string(),
        )
        .unwrap();
        assert_eq!(edge.label, "liked");
        assert_eq!(edge.media, "media_id_1");
        assert_eq!(edge.tag, "tag_id_1");
    }

    #[test]
    fn test_edge_new_empty_label() {
        assert_eq!(
            Edge::new(
                "".to_string(),
                "media_id_1".to_string(),
                "tag_id_1".to_string()
            ),
            Err(EdgeError::EmptyLabel)
        );
    }

    #[test]
    fn test_edge_new_empty_media() {
        assert_eq!(
            Edge::new("liked".to_string(), "".to_string(), "tag_id_1".to_string()),
            Err(EdgeError::EmptyMedia)
        );
    }

    #[test]
    fn test_edge_new_empty_tag() {
        assert_eq!(
            Edge::new(
                "liked".to_string(),
                "media_id_1".to_string(),
                "".to_string()
            ),
            Err(EdgeError::EmptyTag)
        );
    }
}
