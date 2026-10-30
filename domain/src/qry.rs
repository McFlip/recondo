use crate::model::{Media, MediaId, Tag, TagId, normalize_relationship};
use crate::repository::{MediaRepository, TagRepository};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedTag {
    pub tag: Tag,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipTagGroup {
    pub relationship: String,
    pub tags: Vec<CountedTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaQueryItems {
    Count(usize),
    Items(Vec<Media>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaQueryResult {
    pub relationships: Vec<RelationshipTagGroup>,
    pub media: MediaQueryItems,
}

#[derive(Debug)]
pub enum EnrichedQueryError<MediaError, TagError> {
    MediaRepository(MediaError),
    TagRepository(TagError),
    MissingTag { relationship: String, tag_id: TagId },
}

impl<MediaError: std::fmt::Display, TagError: std::fmt::Display> std::fmt::Display
    for EnrichedQueryError<MediaError, TagError>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MediaRepository(error) => write!(f, "media repository error: {error}"),
            Self::TagRepository(error) => write!(f, "tag repository error: {error}"),
            Self::MissingTag {
                relationship,
                tag_id,
            } => write!(
                f,
                "tag {tag_id} for relationship {relationship} was not found"
            ),
        }
    }
}

impl<MediaError, TagError> std::error::Error for EnrichedQueryError<MediaError, TagError>
where
    MediaError: std::error::Error + 'static,
    TagError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MediaRepository(error) => Some(error),
            Self::TagRepository(error) => Some(error),
            Self::MissingTag { .. } => None,
        }
    }
}

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

/// Removes Media IDs whose adjacency list contains any of the supplied Tag IDs.
pub fn filter_out_tags<R: MediaRepository>(
    media_ids: HashSet<MediaId>,
    tag_ids: HashSet<TagId>,
    repo: &R,
) -> Result<Vec<MediaId>, R::Error> {
    let mut result = Vec::new();
    for media_id in media_ids {
        let adj_list = repo.get_media_adj_list(&media_id)?;
        let has_tag = adj_list
            .unwrap_or_default()
            .iter()
            .any(|(_, tag_id)| tag_ids.contains(tag_id));
        if !has_tag {
            result.push(media_id);
        }
    }
    Ok(result)
}

/// Query function to retrieve the adjacency list of a Tag by its ID.
pub fn get_tag_adj_list<R: TagRepository>(
    tag_id: &str,
    repo: &R,
) -> Result<Option<Vec<(String, String)>>, R::Error> {
    repo.get_tag_adj_list(tag_id)
}

/// Query function to retrieve relationship keys starting with the given prefix.
/// Keep in mind that relationships are normalized.
pub fn get_relationships_by_prefix<R: TagRepository>(
    prefix: &str,
    limit: Option<usize>,
    repo: &R,
) -> Result<Option<Vec<String>>, R::Error> {
    if limit == Some(0) {
        return Ok(None);
    }
    let normalized = normalize_relationship(prefix);
    if normalized.is_empty() {
        return Ok(None);
    }
    repo.get_relationships_by_prefix(&normalized, limit)
}

/// Query function to retrieve the Tags associated with a given relationship.
pub fn get_tags_by_relationship<R: TagRepository>(
    relationship: &str,
    repo: &R,
) -> Result<Option<Vec<Tag>>, R::Error> {
    let normalized = normalize_relationship(relationship);
    if normalized.is_empty() {
        return Ok(None);
    }
    repo.get_tags_by_relationship(&normalized)
}

/// Query function returning the set of Media IDs tagged by every Tag ID passed (logical AND).
pub fn get_media_ids_by_logical_and<R, I, T>(
    tag_ids: I,
    repo: &R,
) -> Result<HashSet<MediaId>, R::Error>
where
    R: TagRepository,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let mut result: Option<HashSet<MediaId>> = None;
    for tag_id in tag_ids {
        let media_ids: HashSet<MediaId> = repo
            .get_tag_adj_list(tag_id.as_ref())?
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, media_id)| MediaId::from_hash(media_id).ok())
            .collect();
        match &mut result {
            None => result = Some(media_ids),
            Some(existing) => existing.retain(|id| media_ids.contains(id)),
        }
    }
    Ok(result.unwrap_or_default())
}

/// Query function returning the set of Media IDs tagged by any Tag ID passed (logical OR).
pub fn get_media_ids_by_logical_or<R, I, T>(
    tag_ids: I,
    repo: &R,
) -> Result<HashSet<MediaId>, R::Error>
where
    R: TagRepository,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let mut result: HashSet<MediaId> = HashSet::new();
    for tag_id in tag_ids {
        let media_ids: HashSet<MediaId> = repo
            .get_tag_adj_list(tag_id.as_ref())?
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, media_id)| MediaId::from_hash(media_id).ok())
            .collect();
        result.extend(media_ids);
    }
    Ok(result)
}

/// Query function applying 2-level logic: logical AND within each inner group of Tag IDs,
/// then logical OR across groups.
pub fn get_media_ids_by_logical_and_then_or<R, I, J, T>(
    groups: I,
    repo: &R,
) -> Result<HashSet<MediaId>, R::Error>
where
    R: TagRepository,
    I: IntoIterator<Item = J>,
    J: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let mut result: HashSet<MediaId> = HashSet::new();
    for group in groups {
        let and_set = get_media_ids_by_logical_and(group, repo)?;
        result.extend(and_set);
    }
    Ok(result)
}

/// Query function applying 2-level logic: logical OR within each inner group of Tag IDs,
/// then logical AND across groups.
pub fn get_media_ids_by_logical_or_then_and<R, I, J, T>(
    groups: I,
    repo: &R,
) -> Result<HashSet<MediaId>, R::Error>
where
    R: TagRepository,
    I: IntoIterator<Item = J>,
    J: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let mut result: Option<HashSet<MediaId>> = None;
    for group in groups {
        let or_set = get_media_ids_by_logical_or(group, repo)?;
        match &mut result {
            None => result = Some(or_set),
            Some(existing) => existing.retain(|id| or_set.contains(id)),
        }
    }
    Ok(result.unwrap_or_default())
}

fn get_relationship_tags<R>(
    edge_counts: HashMap<(String, TagId), usize>,
    repo: &R,
) -> Result<
    Vec<RelationshipTagGroup>,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
{
    let mut by_relationship: BTreeMap<String, Vec<(TagId, usize)>> = BTreeMap::new();
    for ((relationship, tag_id), count) in edge_counts {
        by_relationship
            .entry(relationship)
            .or_default()
            .push((tag_id, count));
    }

    let mut groups = Vec::with_capacity(by_relationship.len());
    for (relationship, tag_counts) in by_relationship {
        let tags = repo
            .get_tags_by_relationship(&relationship)
            .map_err(EnrichedQueryError::TagRepository)?
            .unwrap_or_default();
        let mut tags_by_id: HashMap<String, Tag> = tags
            .into_iter()
            .map(|tag| (tag.id().to_string(), tag))
            .collect();
        let mut counted_tags = Vec::with_capacity(tag_counts.len());
        for (tag_id, count) in tag_counts {
            let tag = tags_by_id
                .remove(&tag_id)
                .ok_or_else(|| EnrichedQueryError::MissingTag {
                    relationship: relationship.clone(),
                    tag_id: tag_id.clone(),
                })?;
            counted_tags.push(CountedTag { tag, count });
        }
        counted_tags.sort_by(|left, right| {
            left.count
                .cmp(&right.count)
                .then_with(|| left.tag.name().cmp(right.tag.name()))
                .then_with(|| left.tag.id().cmp(right.tag.id()))
        });
        groups.push(RelationshipTagGroup {
            relationship,
            tags: counted_tags,
        });
    }
    Ok(groups)
}

fn build_media_query_result<R>(
    media_ids: HashSet<MediaId>,
    search_tag_ids: HashSet<TagId>,
    limit: usize,
    repo: &R,
) -> Result<
    MediaQueryResult,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
{
    let mut edge_counts: HashMap<(String, TagId), usize> = HashMap::new();
    for media_id in &media_ids {
        for (relationship, tag_id) in repo
            .get_media_adj_list(media_id)
            .map_err(EnrichedQueryError::MediaRepository)?
            .unwrap_or_default()
        {
            if !search_tag_ids.contains(&tag_id) {
                *edge_counts.entry((relationship, tag_id)).or_default() += 1;
            }
        }
    }

    let relationships = get_relationship_tags(edge_counts, repo)?;
    let media = if media_ids.len() < limit {
        let mut ids: Vec<MediaId> = media_ids.into_iter().collect();
        ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(media) = repo
                .get_media(&id)
                .map_err(EnrichedQueryError::MediaRepository)?
            {
                items.push(media);
            }
        }
        MediaQueryItems::Items(items)
    } else {
        MediaQueryItems::Count(media_ids.len())
    };

    Ok(MediaQueryResult {
        relationships,
        media,
    })
}

/// Query Media and related Tag counts for Media IDs matching logical AND.
pub fn get_enriched_media_by_logical_and<R, I, T>(
    tag_ids: I,
    limit: usize,
    repo: &R,
) -> Result<
    MediaQueryResult,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let tag_ids: Vec<String> = tag_ids
        .into_iter()
        .map(|tag_id| tag_id.as_ref().to_string())
        .collect();
    let media_ids = get_media_ids_by_logical_and(tag_ids.iter().map(String::as_str), repo)
        .map_err(EnrichedQueryError::TagRepository)?;
    build_media_query_result(media_ids, tag_ids.into_iter().collect(), limit, repo)
}

/// Query Media and related Tag counts for Media IDs matching logical OR.
pub fn get_enriched_media_by_logical_or<R, I, T>(
    tag_ids: I,
    limit: usize,
    repo: &R,
) -> Result<
    MediaQueryResult,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let tag_ids: Vec<String> = tag_ids
        .into_iter()
        .map(|tag_id| tag_id.as_ref().to_string())
        .collect();
    let media_ids = get_media_ids_by_logical_or(tag_ids.iter().map(String::as_str), repo)
        .map_err(EnrichedQueryError::TagRepository)?;
    build_media_query_result(media_ids, tag_ids.into_iter().collect(), limit, repo)
}

/// Query Media and related Tag counts for AND-within-group, OR-across-groups matching.
pub fn get_enriched_media_by_logical_and_then_or<R, I, J, T>(
    groups: I,
    limit: usize,
    repo: &R,
) -> Result<
    MediaQueryResult,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
    I: IntoIterator<Item = J>,
    J: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let groups: Vec<Vec<String>> = groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|tag_id| tag_id.as_ref().to_string())
                .collect()
        })
        .collect();
    let media_ids = get_media_ids_by_logical_and_then_or(
        groups.iter().map(|group| group.iter().map(String::as_str)),
        repo,
    )
    .map_err(EnrichedQueryError::TagRepository)?;
    let search_tag_ids = groups.into_iter().flatten().collect();
    build_media_query_result(media_ids, search_tag_ids, limit, repo)
}

/// Query Media and related Tag counts for OR-within-group, AND-across-groups matching.
pub fn get_enriched_media_by_logical_or_then_and<R, I, J, T>(
    groups: I,
    limit: usize,
    repo: &R,
) -> Result<
    MediaQueryResult,
    EnrichedQueryError<<R as MediaRepository>::Error, <R as TagRepository>::Error>,
>
where
    R: MediaRepository + TagRepository,
    I: IntoIterator<Item = J>,
    J: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    let groups: Vec<Vec<String>> = groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|tag_id| tag_id.as_ref().to_string())
                .collect()
        })
        .collect();
    let media_ids = get_media_ids_by_logical_or_then_and(
        groups.iter().map(|group| group.iter().map(String::as_str)),
        repo,
    )
    .map_err(EnrichedQueryError::TagRepository)?;
    let search_tag_ids = groups.into_iter().flatten().collect();
    build_media_query_result(media_ids, search_tag_ids, limit, repo)
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
        tag_adj_lists: HashMap<String, Vec<(String, String)>>,
        relationship_tags: HashMap<String, Vec<Tag>>,
        should_fail: bool,
        tag_should_fail: bool,
    }

    impl MockMediaRepo {
        fn new() -> Self {
            Self {
                items: HashMap::new(),
                adj_lists: HashMap::new(),
                tag_adj_lists: HashMap::new(),
                relationship_tags: HashMap::new(),
                should_fail: false,
                tag_should_fail: false,
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

        fn with_tag_adj_list(
            mut self,
            tag_id: impl Into<String>,
            list: Vec<(String, String)>,
        ) -> Self {
            self.tag_adj_lists.insert(tag_id.into(), list);
            self
        }

        fn with_relationship_tags(mut self, relationship: &str, tags: Vec<Tag>) -> Self {
            self.relationship_tags
                .insert(relationship.to_string(), tags);
            self
        }

        fn failing() -> Self {
            Self {
                items: HashMap::new(),
                adj_lists: HashMap::new(),
                tag_adj_lists: HashMap::new(),
                relationship_tags: HashMap::new(),
                should_fail: true,
                tag_should_fail: true,
            }
        }

        fn media_failing() -> Self {
            Self {
                items: HashMap::new(),
                adj_lists: HashMap::new(),
                tag_adj_lists: HashMap::new(),
                relationship_tags: HashMap::new(),
                should_fail: true,
                tag_should_fail: false,
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

        fn save_media_with_tags(
            &self,
            media: &Media,
            src_path: &Path,
            dest_path: &Path,
            _tags: &[(String, Tag)],
        ) -> Result<(), SaveMediaError<Self::Error>> {
            self.save_media(media, src_path, dest_path)
        }

        fn apply_upload_tags_to_duplicate(
            &self,
            _media_id: &MediaId,
            _tags: &[(String, Tag)],
        ) -> Result<(), Self::Error> {
            if self.should_fail {
                Err(MockError)
            } else {
                Ok(())
            }
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

    impl TagRepository for MockMediaRepo {
        type Error = MockError;

        fn init(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn tag_exists(&self, id: &str) -> Result<bool, Self::Error> {
            if self.tag_should_fail {
                return Err(MockError);
            }
            Ok(self.tag_adj_lists.contains_key(id))
        }

        fn add_tag_edge(
            &self,
            _media_id: &str,
            _tag: &Tag,
            _relationship: &str,
        ) -> Result<(), Self::Error> {
            if self.tag_should_fail {
                return Err(MockError);
            }
            Ok(())
        }

        fn get_tag_adj_list(
            &self,
            tag_id: &str,
        ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
            if self.tag_should_fail {
                return Err(MockError);
            }
            Ok(self.tag_adj_lists.get(tag_id).cloned())
        }

        fn get_relationships_by_prefix(
            &self,
            prefix: &str,
            limit: Option<usize>,
        ) -> Result<Option<Vec<String>>, Self::Error> {
            if self.tag_should_fail {
                return Err(MockError);
            }
            if limit == Some(0) {
                return Ok(None);
            }
            let mut relationships: Vec<String> = self
                .relationship_tags
                .keys()
                .filter(|relationship| relationship.starts_with(prefix))
                .cloned()
                .collect();
            relationships.sort();
            if let Some(limit) = limit {
                relationships.truncate(limit);
            }
            if relationships.is_empty() {
                Ok(None)
            } else {
                Ok(Some(relationships))
            }
        }

        fn get_tags_by_relationship(
            &self,
            relationship: &str,
        ) -> Result<Option<Vec<Tag>>, Self::Error> {
            if self.tag_should_fail {
                return Err(MockError);
            }
            Ok(self.relationship_tags.get(relationship).cloned())
        }
    }

    struct MockTagRepo {
        tags: HashMap<String, Vec<(String, String)>>,
        relationships: Vec<String>,
        relationship_tags: HashMap<String, Vec<Tag>>,
        should_fail: bool,
    }

    impl MockTagRepo {
        fn new() -> Self {
            Self {
                tags: HashMap::new(),
                relationships: Vec::new(),
                relationship_tags: HashMap::new(),
                should_fail: false,
            }
        }

        fn with_adj_list(mut self, tag_id: impl Into<String>, list: Vec<(String, String)>) -> Self {
            self.tags.insert(tag_id.into(), list);
            self
        }

        fn with_relationships(mut self, rels: Vec<&str>) -> Self {
            self.relationships = rels.into_iter().map(normalize_relationship).collect();
            self
        }

        fn failing() -> Self {
            Self {
                tags: HashMap::new(),
                relationships: Vec::new(),
                relationship_tags: HashMap::new(),
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
            _tag: &Tag,
            _relationship: &str,
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

        fn get_relationships_by_prefix(
            &self,
            prefix: &str,
            limit: Option<usize>,
        ) -> Result<Option<Vec<String>>, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            if limit == Some(0) {
                return Ok(None);
            }
            let iter = self
                .relationships
                .iter()
                .filter(|r| r.starts_with(prefix))
                .cloned();
            let matches: Vec<String> = match limit {
                Some(lim) => iter.take(lim).collect(),
                None => iter.collect(),
            };
            if matches.is_empty() {
                Ok(None)
            } else {
                Ok(Some(matches))
            }
        }

        fn get_tags_by_relationship(
            &self,
            relationship: &str,
        ) -> Result<Option<Vec<Tag>>, Self::Error> {
            if self.should_fail {
                return Err(MockError);
            }
            match self.relationship_tags.get(relationship) {
                Some(tags) if !tags.is_empty() => Ok(Some(tags.clone())),
                _ => Ok(None),
            }
        }
    }

    fn logical_query_repo() -> MockTagRepo {
        MockTagRepo::new()
            .with_adj_list(
                "tag_a",
                vec![
                    ("rel".into(), "media_1".into()),
                    ("rel".into(), "media_2".into()),
                ],
            )
            .with_adj_list(
                "tag_b",
                vec![
                    ("rel".into(), "media_2".into()),
                    ("rel".into(), "media_3".into()),
                ],
            )
            .with_adj_list(
                "tag_c",
                vec![
                    ("rel".into(), "media_3".into()),
                    ("rel".into(), "media_4".into()),
                ],
            )
    }

    fn media_ids(ids: &[&str]) -> HashSet<MediaId> {
        ids.iter()
            .map(|id| MediaId::from_hash(*id).unwrap())
            .collect()
    }

    fn enriched_query_repo() -> MockMediaRepo {
        let media_a = Media::new(
            MediaId::from_hash("media-a").unwrap(),
            Url::parse("file:///media-a.png").unwrap(),
            None,
        );
        let media_b = Media::new(
            MediaId::from_hash("media-b").unwrap(),
            Url::parse("file:///media-b.png").unwrap(),
            None,
        );
        MockMediaRepo::new()
            .with_media(media_a)
            .with_media(media_b)
            .with_tag_adj_list(
                "search_a",
                vec![
                    ("search".into(), "media-a".into()),
                    ("search".into(), "media-b".into()),
                    ("search".into(), "missing-media".into()),
                ],
            )
            .with_tag_adj_list(
                "search_b",
                vec![
                    ("search".into(), "media-a".into()),
                    ("search".into(), "media-b".into()),
                    ("search".into(), "missing-media".into()),
                ],
            )
            .with_adj_list(
                "media-a",
                vec![
                    ("subject".into(), "search_a".into()),
                    ("subject".into(), "rare".into()),
                    ("subject".into(), "shared".into()),
                    ("place".into(), "coast".into()),
                ],
            )
            .with_adj_list(
                "media-b",
                vec![
                    ("subject".into(), "search_b".into()),
                    ("subject".into(), "shared".into()),
                    ("place".into(), "coast".into()),
                ],
            )
            .with_adj_list(
                "missing-media",
                vec![
                    ("subject".into(), "search_a".into()),
                    ("subject".into(), "search_b".into()),
                    ("subject".into(), "shared".into()),
                ],
            )
            .with_relationship_tags(
                "subject",
                vec![
                    Tag::from_parts("search_a", "Search A"),
                    Tag::from_parts("search_b", "Search B"),
                    Tag::from_parts("rare", "Rare"),
                    Tag::from_parts("shared", "Shared"),
                ],
            )
            .with_relationship_tags("place", vec![Tag::from_parts("coast", "Coast")])
    }

    #[test]
    fn test_get_enriched_media_by_logical_and_aggregates_relationship_tags() {
        let repo = enriched_query_repo();

        let result = get_enriched_media_by_logical_and(["search_a", "search_b"], 4, &repo).unwrap();

        assert_eq!(
            result.relationships,
            vec![
                RelationshipTagGroup {
                    relationship: "place".into(),
                    tags: vec![CountedTag {
                        tag: Tag::from_parts("coast", "Coast"),
                        count: 2,
                    }],
                },
                RelationshipTagGroup {
                    relationship: "subject".into(),
                    tags: vec![
                        CountedTag {
                            tag: Tag::from_parts("rare", "Rare"),
                            count: 1,
                        },
                        CountedTag {
                            tag: Tag::from_parts("shared", "Shared"),
                            count: 3,
                        },
                    ],
                },
            ]
        );
        assert_eq!(
            result.media,
            MediaQueryItems::Items(vec![
                repo.items.get("media-a").unwrap().clone(),
                repo.items.get("media-b").unwrap().clone(),
            ])
        );

        let at_limit =
            get_enriched_media_by_logical_and(["search_a", "search_b"], 3, &repo).unwrap();
        assert_eq!(at_limit.media, MediaQueryItems::Count(3));
    }

    #[test]
    fn test_enriched_query_wrappers_preserve_other_query_strategies() {
        let repo = enriched_query_repo();

        let logical_or =
            get_enriched_media_by_logical_or(["search_a", "search_b"], 3, &repo).unwrap();
        assert_eq!(logical_or.media, MediaQueryItems::Count(3));

        let and_then_or =
            get_enriched_media_by_logical_and_then_or([["search_a"], ["search_b"]], 3, &repo)
                .unwrap();
        assert_eq!(and_then_or.media, MediaQueryItems::Count(3));

        let or_then_and = get_enriched_media_by_logical_or_then_and(
            vec![vec!["search_a", "search_b"], vec!["search_a"]],
            3,
            &repo,
        )
        .unwrap();
        assert_eq!(or_then_and.media, MediaQueryItems::Count(3));
    }

    #[test]
    fn test_enriched_query_propagates_repository_errors() {
        assert!(matches!(
            get_enriched_media_by_logical_and(["search_a"], 5, &MockMediaRepo::failing()),
            Err(EnrichedQueryError::TagRepository(_))
        ));

        let repo = MockMediaRepo::media_failing()
            .with_tag_adj_list("search_a", vec![("search".into(), "media-a".into())]);
        assert!(matches!(
            get_enriched_media_by_logical_and(["search_a"], 5, &repo),
            Err(EnrichedQueryError::MediaRepository(_))
        ));
    }

    #[test]
    fn test_get_media_ids_by_logical_and() {
        let repo = logical_query_repo();

        let result = get_media_ids_by_logical_and(["tag_a", "tag_b"], &repo).unwrap();

        assert_eq!(result, media_ids(&["media_2"]));
    }

    #[test]
    fn test_get_media_ids_by_logical_or() {
        let repo = logical_query_repo();

        let result = get_media_ids_by_logical_or(["tag_a", "tag_b"], &repo).unwrap();

        assert_eq!(result, media_ids(&["media_1", "media_2", "media_3"]));
    }

    #[test]
    fn test_get_media_ids_by_logical_and_then_or() {
        let repo = logical_query_repo();

        let result = get_media_ids_by_logical_and_then_or(
            vec![vec!["tag_a", "tag_b"], vec!["tag_c"]],
            &repo,
        )
        .unwrap();

        assert_eq!(result, media_ids(&["media_2", "media_3", "media_4"]));
    }

    #[test]
    fn test_get_media_ids_by_logical_or_then_and() {
        let repo = logical_query_repo();

        let result =
            get_media_ids_by_logical_or_then_and([["tag_a", "tag_b"], ["tag_b", "tag_c"]], &repo)
                .unwrap();

        assert_eq!(result, media_ids(&["media_2", "media_3"]));
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
    fn test_filter_out_tags() {
        let tagged_id = MediaId::from_hash("tagged").unwrap();
        let untagged_id = MediaId::from_hash("untagged").unwrap();
        let missing_adj_list_id = MediaId::from_hash("missing").unwrap();
        let media_ids = HashSet::from([
            tagged_id.clone(),
            untagged_id.clone(),
            missing_adj_list_id.clone(),
        ]);
        let tag_ids = HashSet::from(["tag_a".to_string(), "tag_b".to_string()]);
        let repo = MockMediaRepo::new()
            .with_adj_list("tagged", vec![("relationship".into(), "tag_b".into())])
            .with_adj_list("untagged", vec![("relationship".into(), "tag_c".into())]);

        let result = filter_out_tags(media_ids, tag_ids, &repo).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&untagged_id));
        assert!(result.contains(&missing_adj_list_id));
        assert!(!result.contains(&tagged_id));
    }

    #[test]
    fn test_filter_out_tags_error() {
        let media_ids = HashSet::from([MediaId::from_hash("media123").unwrap()]);
        let repo = MockMediaRepo::failing();

        assert!(filter_out_tags(media_ids, HashSet::new(), &repo).is_err());
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

    #[test]
    fn test_get_relationships_by_prefix_found() {
        let repo = MockTagRepo::new().with_relationships(vec!["to do", "total recall", "unique"]);

        let result = get_relationships_by_prefix("to", None, &repo).unwrap();
        assert_eq!(
            result,
            Some(vec!["to do".to_string(), "total recall".to_string()])
        );
    }

    #[test]
    fn test_get_relationships_by_prefix_with_limit() {
        let repo =
            MockTagRepo::new().with_relationships(vec!["to do", "total recall", "to another"]);

        let result = get_relationships_by_prefix("to", Some(2), &repo).unwrap();
        assert_eq!(
            result,
            Some(vec!["to do".to_string(), "total recall".to_string()])
        );

        let result_zero = get_relationships_by_prefix("to", Some(0), &repo).unwrap();
        assert_eq!(result_zero, None);
    }

    #[test]
    fn test_get_relationships_by_prefix_normalizes_search() {
        let repo = MockTagRepo::new().with_relationships(vec!["  TO DO  ", "liked"]);

        let result = get_relationships_by_prefix("  TO  ", None, &repo).unwrap();
        assert_eq!(result, Some(vec!["to do".to_string()]));
    }

    #[test]
    fn test_get_relationships_by_prefix_empty_or_whitespace() {
        let repo = MockTagRepo::new().with_relationships(vec!["to do"]);

        assert_eq!(get_relationships_by_prefix("", None, &repo).unwrap(), None);
        assert_eq!(
            get_relationships_by_prefix("   ", None, &repo).unwrap(),
            None
        );
    }

    #[test]
    fn test_get_relationships_by_prefix_not_found() {
        let repo = MockTagRepo::new().with_relationships(vec!["to do"]);

        let result = get_relationships_by_prefix("nomatch", None, &repo).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_relationships_by_prefix_error() {
        let repo = MockTagRepo::failing();

        let result = get_relationships_by_prefix("to", None, &repo);
        assert!(result.is_err());
    }
}
