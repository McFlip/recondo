use recondo_domain::{
    cmd::{add_tag_to_media, create_media_from_file},
    model::{Media, MediaId, MediaOpts, Tag},
    qry::{
        CountedTag, MediaQueryItems, RelationshipTagGroup, filter_out_tags,
        get_enriched_media_by_logical_and, get_enriched_media_by_logical_and_then_or,
        get_enriched_media_by_logical_or, get_enriched_media_by_logical_or_then_and,
        get_media_adj_list, get_media_by_id, get_media_ids_by_logical_and,
        get_media_ids_by_logical_and_then_or, get_media_ids_by_logical_or,
        get_media_ids_by_logical_or_then_and, get_relationships_by_prefix, get_tag_adj_list,
        get_tags_by_relationship,
    },
    repository::{MediaRepository, SaveMediaError, TagRepository},
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use url::Url;

#[derive(Debug)]
struct TestError;

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "test repository error")
    }
}

impl Error for TestError {}

#[derive(Default)]
struct TestRepo {
    media: HashMap<String, Media>,
    media_edges: HashMap<String, Vec<(String, String)>>,
    tag_edges: HashMap<String, Vec<(String, String)>>,
    relationships: Vec<String>,
    relationship_tags: HashMap<String, Vec<Tag>>,
    saved_media: RefCell<Vec<Media>>,
    saved_edges: RefCell<Vec<(String, Tag, String)>>,
}

impl MediaRepository for TestRepo {
    type Error = TestError;

    fn init(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn media_exists(&self, id: &MediaId) -> Result<bool, Self::Error> {
        Ok(self.media.contains_key(id.as_str()))
    }

    fn save_media(
        &self,
        media: &Media,
        _src_path: &Path,
        _dest_path: &Path,
    ) -> Result<(), SaveMediaError<Self::Error>> {
        self.saved_media.borrow_mut().push(media.clone());
        Ok(())
    }

    fn save_media_record(&self, _media: &Media) -> Result<(), Self::Error> {
        Ok(())
    }

    fn get_media(&self, id: &MediaId) -> Result<Option<Media>, Self::Error> {
        Ok(self.media.get(id.as_str()).cloned())
    }

    fn get_media_adj_list(
        &self,
        id: &MediaId,
    ) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        Ok(self.media_edges.get(id.as_str()).cloned())
    }
}

impl TagRepository for TestRepo {
    type Error = TestError;

    fn init(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn tag_exists(&self, id: &str) -> Result<bool, Self::Error> {
        Ok(self
            .relationship_tags
            .values()
            .flatten()
            .any(|tag| tag.id() == id))
    }

    fn add_tag_edge(
        &self,
        media_id: &str,
        tag: &Tag,
        relationship: &str,
    ) -> Result<(), Self::Error> {
        self.saved_edges.borrow_mut().push((
            media_id.to_string(),
            tag.clone(),
            relationship.to_string(),
        ));
        Ok(())
    }

    fn get_tag_adj_list(&self, tag_id: &str) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        Ok(self.tag_edges.get(tag_id).cloned())
    }

    fn get_relationships_by_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Self::Error> {
        let matches: Vec<String> = self
            .relationships
            .iter()
            .filter(|relationship| relationship.starts_with(prefix))
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect();
        Ok((!matches.is_empty()).then_some(matches))
    }

    fn get_tags_by_relationship(
        &self,
        relationship: &str,
    ) -> Result<Option<Vec<Tag>>, Self::Error> {
        Ok(self.relationship_tags.get(relationship).cloned())
    }
}

fn media_id(id: &str) -> MediaId {
    MediaId::from_hash(id).unwrap()
}

fn media(id: &str) -> Media {
    Media::new(
        media_id(id),
        Url::parse(&format!("file:///assets/{id}.png")).unwrap(),
        Some(MediaOpts {
            title: Some(id.to_string()),
            mime_type: Some("image/png".to_string()),
            file_size: Some(10),
            view_count: Some(1),
        }),
    )
}

fn id_set(ids: &[&str]) -> HashSet<MediaId> {
    ids.iter().map(|id| media_id(id)).collect()
}

fn seeded_repo() -> TestRepo {
    let mut repo = TestRepo::default();
    for id in ["m1", "m2", "m3", "m4"] {
        repo.media.insert(id.to_string(), media(id));
    }
    repo.media_edges = HashMap::from([
        (
            "m1".to_string(),
            vec![
                ("subject".to_string(), "a".to_string()),
                ("place".to_string(), "coast".to_string()),
            ],
        ),
        (
            "m2".to_string(),
            vec![
                ("subject".to_string(), "a".to_string()),
                ("subject".to_string(), "b".to_string()),
                ("place".to_string(), "coast".to_string()),
            ],
        ),
        (
            "m3".to_string(),
            vec![
                ("subject".to_string(), "b".to_string()),
                ("place".to_string(), "city".to_string()),
            ],
        ),
        (
            "m4".to_string(),
            vec![
                ("subject".to_string(), "c".to_string()),
                ("place".to_string(), "city".to_string()),
            ],
        ),
    ]);
    repo.tag_edges = HashMap::from([
        (
            "a".to_string(),
            vec![
                ("subject".to_string(), "m1".to_string()),
                ("subject".to_string(), "m2".to_string()),
            ],
        ),
        (
            "b".to_string(),
            vec![
                ("subject".to_string(), "m2".to_string()),
                ("subject".to_string(), "m3".to_string()),
            ],
        ),
        (
            "c".to_string(),
            vec![("subject".to_string(), "m4".to_string())],
        ),
    ]);
    repo.relationships = vec!["place".to_string(), "subject".to_string()];
    repo.relationship_tags = HashMap::from([
        (
            "place".to_string(),
            vec![
                Tag::from_parts("coast", "Coast"),
                Tag::from_parts("city", "City"),
            ],
        ),
        (
            "subject".to_string(),
            vec![
                Tag::from_parts("a", "A"),
                Tag::from_parts("b", "B"),
                Tag::from_parts("c", "C"),
            ],
        ),
    ]);
    repo
}

#[test]
fn public_commands_create_media_and_add_tags() {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "recondo_domain_integration_{}_{}.bin",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let contents = b"integration media";
    fs::write(&path, contents).unwrap();
    let repo = TestRepo::default();

    let created = create_media_from_file(path.clone(), &repo)
        .unwrap()
        .unwrap();

    assert_eq!(
        created.id.as_str(),
        blake3::hash(contents).to_hex().as_str()
    );
    assert_eq!(repo.saved_media.borrow().as_slice(), &[created]);
    fs::remove_file(path).unwrap();

    let tag = add_tag_to_media("m1", "  Landscape ", "LIKED", &repo).unwrap();
    assert_eq!(tag.name(), "landscape");
    assert_eq!(
        repo.saved_edges.borrow().as_slice(),
        &[("m1".to_string(), tag, "LIKED".to_string())]
    );
}

#[test]
fn public_basic_queries_cover_media_tags_and_relationships() {
    let repo = seeded_repo();
    let m2 = media_id("m2");

    assert_eq!(get_media_by_id(&m2, &repo).unwrap(), Some(media("m2")));
    assert_eq!(
        get_media_adj_list(&m2, &repo).unwrap(),
        repo.media_edges.get("m2").cloned()
    );
    assert_eq!(
        filter_out_tags(
            id_set(&["m1", "m2", "m4"]),
            HashSet::from(["a".to_string()]),
            &repo,
        )
        .unwrap()
        .into_iter()
        .collect::<HashSet<_>>(),
        id_set(&["m4"])
    );
    assert_eq!(
        get_tag_adj_list("a", &repo).unwrap(),
        repo.tag_edges.get("a").cloned()
    );
    assert_eq!(
        get_relationships_by_prefix(" PL ", Some(1), &repo).unwrap(),
        Some(vec!["place".to_string()])
    );
    assert_eq!(
        get_tags_by_relationship(" PLACE ", &repo).unwrap(),
        repo.relationship_tags.get("place").cloned()
    );
}

#[test]
fn public_logical_and_enriched_queries_cover_all_strategies() {
    let repo = seeded_repo();

    assert_eq!(
        get_media_ids_by_logical_and(["a", "b"], &repo).unwrap(),
        id_set(&["m2"])
    );
    assert_eq!(
        get_media_ids_by_logical_or(["a", "b"], &repo).unwrap(),
        id_set(&["m1", "m2", "m3"])
    );
    assert_eq!(
        get_media_ids_by_logical_and_then_or(vec![vec!["a", "b"], vec!["c"]], &repo).unwrap(),
        id_set(&["m2", "m4"])
    );
    assert_eq!(
        get_media_ids_by_logical_or_then_and([["a", "b"], ["b", "c"]], &repo).unwrap(),
        id_set(&["m2", "m3"])
    );

    let and = get_enriched_media_by_logical_and(["a", "b"], 10, &repo).unwrap();
    assert_eq!(and.media, MediaQueryItems::Items(vec![media("m2")]));
    assert_eq!(
        and.relationships,
        vec![RelationshipTagGroup {
            relationship: "place".to_string(),
            tags: vec![CountedTag {
                tag: Tag::from_parts("coast", "Coast"),
                count: 1,
            }],
        }]
    );

    let or = get_enriched_media_by_logical_or(["a", "b"], 10, &repo).unwrap();
    assert_eq!(
        or.media,
        MediaQueryItems::Items(vec![media("m1"), media("m2"), media("m3")])
    );
    assert_eq!(
        get_enriched_media_by_logical_and_then_or(vec![vec!["a", "b"], vec!["c"]], 10, &repo)
            .unwrap()
            .media,
        MediaQueryItems::Items(vec![media("m2"), media("m4")])
    );
    assert_eq!(
        get_enriched_media_by_logical_or_then_and([["a", "b"], ["b", "c"]], 10, &repo)
            .unwrap()
            .media,
        MediaQueryItems::Items(vec![media("m2"), media("m3")])
    );
}
