use std::error::Error;
use std::fmt;
use std::str::FromStr;

use unicode_normalization::UnicodeNormalization;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct Media {
    pub id: MediaId,
    pub url: Url,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
    pub view_count: u64,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

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
}
