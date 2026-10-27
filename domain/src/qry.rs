use crate::model::{Media, MediaId};
use crate::repository::{MediaRepository, TagRepository};

/// Query function to retrieve a Media item by its ID.
pub fn get_media_by_id<R: MediaRepository>(
    id: &MediaId,
    repo: &R,
) -> Result<Option<Media>, R::Error> {
    repo.get_media(id)
}

/// Query function to retrieve the adjacency list of a Media item by its ID.
pub fn get_media_adj_list<R: MediaRepository>(
    id: &MediaId,
    repo: &R,
) -> Result<Option<Vec<(String, String)>>, R::Error> {
    repo.get_media_adj_list(id)
}

/// Query function to retrieve the adjacency list of a Tag by its ID.
pub fn get_tag_adj_list<R: TagRepository>(
    tag_id: &str,
    repo: &R,
) -> Result<Option<Vec<(String, String)>>, R::Error> {
    repo.get_tag_adj_list(tag_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MediaOpts;
    use crate::repository::SaveMediaError;
    use std::collections::HashMap;
    use std::error::Error as StdError;
    use std::fmt;
    use std::path::Path;
    use url::Url;

    #[derive(Debug)]
    struct MockError;

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "mock error")
        }
    }

    impl StdError for MockError {}

    struct MockMediaRepo {
        items: HashMap<String, Media>,
        adj_lists: HashMap<String, Vec<(String, String)>>,
        should_fail: bool,
    }

    impl MockMediaRepo {
        fn new() -> Self {
            Self {
                items: HashMap::new(),
                adj_lists: HashMap::new(),
                should_fail: false,
            }
        }

        fn with_media(mut self, media: Media) -> Self {
            self.items.insert(media.id.as_str().to_string(), media);
            self
        }

        fn with_adj_list(mut self, id: impl Into<String>, list: Vec<(String, String)>) -> Self {
            self.adj_lists.insert(id.into(), list);
            self
        }

        fn failing() -> Self {
            Self {
                items: HashMap::new(),
                adj_lists: HashMap::new(),
                should_fail: true,
            }
        }
    }

    impl MediaRepository for MockMediaRepo {
        type Error = MockError;

        fn init(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(self.items.contains_key(id.as_str()))
        }

        fn save_media(
            &self,
            media: &Media,
            _src_path: &Path,
            _dest_path: &Path,
        ) -> Result<(), SaveMediaError<Self::Error>> {
            if self.should_fail {
                return Err(SaveMediaError::Repository(MockError));
            }
            let _ = media;
            Ok(())
        }

        fn save_media_record(&self, media: &Media) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            let _ = media;
            Ok(())
        }

        fn get_media(&self, id: &MediaId) -> Result<Option<Media>, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(self.items.get(id.as_str()).cloned())
        }

        fn get_media_adj_list(
            &self,
            id: &MediaId,
        ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(self.adj_lists.get(id.as_str()).cloned())
        }
    }

    struct MockTagRepo {
        tags: HashMap<String, Vec<(String, String)>>,
        should_fail: bool,
    }

    impl MockTagRepo {
        fn new() -> Self {
            Self {
                tags: HashMap::new(),
                should_fail: false,
            }
        }

        fn with_adj_list(mut self, tag_id: impl Into<String>, list: Vec<(String, String)>) -> Self {
            self.tags.insert(tag_id.into(), list);
            self
        }

        fn failing() -> Self {
            Self {
                tags: HashMap::new(),
                should_fail: true,
            }
        }
    }

    impl TagRepository for MockTagRepo {
        type Error = MockError;

        fn init(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn tag_exists(&self, id: &str) -> Result<bool, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(self.tags.contains_key(id))
        }

        fn add_tag_edge(
            &self,
            _media_id: &str,
            _tag_id: &str,
            _label: &str,
        ) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(())
        }

        fn get_tag_adj_list(
            &self,
            tag_id: &str,
        ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(self.tags.get(tag_id).cloned())
        }

        fn add_tag_to_relationship(&self, _relationship: &str, _tag_id: &str) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            Ok(())
        }
    }

    #[test]
    fn test_get_media_by_id_found() {
        let id = MediaId::from_hash("abcd1234ef").unwrap();
        let url = Url::parse("file:///assets/test.png").unwrap();
        let media = Media::new(
            id.clone(),
            url,
            Some(MediaOpts {
                title: Some("Test Media".to_string()),
                mime_type: Some("image/png".to_string()),
                file_size: Some(1024),
                view_count: Some(5),
            }),
        );
        let repo = MockMediaRepo::new().with_media(media.clone());

        let result = get_media_by_id(&id, &repo).unwrap();
        assert!(result.is_some());
        let found = result.unwrap();
        assert_eq!(found.id, id);
        assert_eq!(found.title, Some("Test Media".to_string()));
        assert_eq!(found.mime_type, Some("image/png".to_string()));
        assert_eq!(found.file_size, Some(1024));
        assert_eq!(found.view_count, 5);
    }

    #[test]
    fn test_get_media_by_id_not_found() {
        let id = MediaId::from_hash("notfound123").unwrap();
        let repo = MockMediaRepo::new();

        let result = get_media_by_id(&id, &repo).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_media_by_id_error() {
        let id = MediaId::from_hash("someid").unwrap();
        let repo = MockMediaRepo::failing();

        let result = get_media_by_id(&id, &repo);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_media_adj_list_found() {
        let id = MediaId::from_hash("media123").unwrap();
        let edges = vec![
            ("to do".to_string(), "tag_a".to_string()),
            ("liked".to_string(), "tag_b".to_string()),
        ];
        let repo = MockMediaRepo::new().with_adj_list("media123", edges.clone());

        let result = get_media_adj_list(&id, &repo).unwrap();
        assert_eq!(result, Some(edges));
    }

    #[test]
    fn test_get_media_adj_list_not_found() {
        let id = MediaId::from_hash("media123").unwrap();
        let repo = MockMediaRepo::new();

        let result = get_media_adj_list(&id, &repo).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_media_adj_list_error() {
        let id = MediaId::from_hash("media123").unwrap();
        let repo = MockMediaRepo::failing();

        let result = get_media_adj_list(&id, &repo);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_tag_adj_list_found() {
        let edges = vec![
            ("to do".to_string(), "media_a".to_string()),
            ("to do".to_string(), "media_b".to_string()),
        ];
        let repo = MockTagRepo::new().with_adj_list("tag123", edges.clone());

        let result = get_tag_adj_list("tag123", &repo).unwrap();
        assert_eq!(result, Some(edges));
    }

    #[test]
    fn test_get_tag_adj_list_not_found() {
        let repo = MockTagRepo::new();

        let result = get_tag_adj_list("tag123", &repo).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_tag_adj_list_error() {
        let repo = MockTagRepo::failing();

        let result = get_tag_adj_list("tag123", &repo);
        assert!(result.is_err());
    }
}
