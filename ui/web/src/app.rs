use std::convert::Infallible;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use async_stream::stream;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Multipart, Path, Query, RawQuery, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
};
use datastar::{
    consts::ElementPatchMode,
    prelude::{PatchElements, PatchSignals},
};
use recondo_dal::media::RedbMediaRepository;
use recondo_domain::cmd::{CreateMediaOutcome, create_media_from_file_in_with_tags};
use recondo_domain::model::{MediaId, Tag, normalize_relationship};
use recondo_domain::repository::{MediaRepository, TagRepository};
use recondo_domain::{
    get_enriched_media_by_logical_and, get_enriched_media_by_logical_or,
    get_relationships_by_prefix, get_tag_adj_list, get_tags_by_relationship,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio_stream::iter;

use crate::pages::explore;
use crate::pages::landing::{self, DemoFilter};
use crate::pages::media;
use crate::pages::upload;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<RedbMediaRepository>,
    pub assets_dir: PathBuf,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(landing_page))
        .route("/explore", get(explore_page))
        .route("/explore/search", get(search_relationships))
        .route("/explore/relationship", get(show_relationship))
        .route("/explore/submit", get(submit_relationship_search))
        .route("/media", get(media_page))
        .route("/media/relationship-search", get(media_picker_search))
        .route("/media/relationship", get(media_picker_relationship))
        .route(
            "/media/relationship-submit",
            get(media_picker_submit_relationship),
        )
        .route("/asset/{id}", get(media_asset_by_id))
        .route("/upload", get(upload_page))
        .route("/upload", post(upload_handler))
        .route(
            "/upload/relationship-search",
            get(search_upload_relationships),
        )
        .route("/upload/relationship", get(show_upload_relationship))
        .route("/upload.js", get(upload_script))
        .route("/media.js", get(media_script))
        .route("/demo/{filter}", get(filter_demo))
        .route("/styles.css", get(stylesheet))
        .route("/media/{filename}", get(media_asset))
        .with_state(state)
        .layer(body_limit_layer())
}

/// Upload size cap from `RECONDO_MAX_UPLOAD_MB` (whole mebibytes).
///
/// Unset or invalid → unlimited; the handler streams uploads to temp files,
/// so memory stays bounded regardless of the cap.
fn body_limit_layer() -> DefaultBodyLimit {
    match std::env::var("RECONDO_MAX_UPLOAD_MB") {
        Ok(raw) => match raw.trim().parse::<usize>() {
            Ok(mb) => DefaultBodyLimit::max(mb.saturating_mul(1024 * 1024)),
            Err(_) => {
                eprintln!(
                    "warning: invalid RECONDO_MAX_UPLOAD_MB {raw:?} (expected integer MB); \
                     uploads unlimited"
                );
                DefaultBodyLimit::disable()
            }
        },
        Err(_) => DefaultBodyLimit::disable(),
    }
}

async fn landing_page() -> maud::Markup {
    landing::render()
}

async fn upload_page() -> maud::Markup {
    upload::render()
}

async fn upload_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../static/upload.js"),
    )
}

async fn media_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../static/media.js"),
    )
}

async fn search_upload_relationships(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let prefix = params.get("prefix").cloned().unwrap_or_default();
    let query_prefix = prefix.clone();
    let repo = state.repo;
    let result = tokio::task::spawn_blocking(move || {
        get_relationships_by_prefix(&query_prefix, Some(12), repo.as_ref())
    })
    .await;

    let markup = match result {
        Ok(Ok(relationships)) => {
            upload::relationship_suggestions(&prefix, relationships.as_deref().unwrap_or_default())
        }
        Ok(Err(error)) => {
            eprintln!("Error searching upload relationships by prefix {prefix:?}: {error}");
            upload::relationship_error(&format!("Could not search relationships: {error}"))
        }
        Err(error) => {
            eprintln!("Upload relationship search task failed for {prefix:?}: {error}");
            upload::relationship_error(&format!("Relationship search failed: {error}"))
        }
    };
    patch_response("#upload-relationship-suggestions", markup)
}

async fn show_upload_relationship(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let relationship = params.get("name").cloned().unwrap_or_default();
    let markup = if normalize_relationship(&relationship).is_empty() {
        upload::relationship_tags_error("Choose a relationship to see its tags.")
    } else {
        let query_relationship = relationship.clone();
        let repo = state.repo;
        match tokio::task::spawn_blocking(move || {
            get_tags_by_relationship(&query_relationship, repo.as_ref())
                .map(|tags| tags.unwrap_or_default())
                .map_err(|error| error.to_string())
        })
        .await
        {
            Ok(Ok(tags)) => upload::relationship_tags(&relationship, &tags),
            Ok(Err(error)) => {
                eprintln!("Error loading upload tags for relationship {relationship:?}: {error}");
                upload::relationship_tags_error(&format!(
                    "Could not load relationship tags: {error}"
                ))
            }
            Err(error) => {
                eprintln!("Upload relationship tag lookup task failed: {error}");
                upload::relationship_tags_error(&format!("Relationship tag lookup failed: {error}"))
            }
        }
    };
    patch_response("#upload-relationship-details", markup)
}

async fn explore_page() -> maud::Markup {
    explore::render()
}

async fn media_page(State(state): State<AppState>, RawQuery(raw_query): RawQuery) -> maud::Markup {
    let params = parse_media_query(raw_query.as_deref().unwrap_or_default());
    let repo = state.repo;

    if params.tag_ids.is_empty() {
        return media::render(
            &[],
            params.limit,
            params.operation,
            None,
            None,
            "Select a tag from Explore to find connected media.",
        );
    }

    let placeholder_tags = params
        .tag_ids
        .iter()
        .map(|id| Tag::from_parts(id.clone(), format!("Unknown tag {}", short_id(id))))
        .collect::<Vec<_>>();
    let tag_ids = params.tag_ids.clone();
    let limit = params.limit;
    let operation = params.operation;
    let result = tokio::task::spawn_blocking(move || {
        let query = match operation {
            media::Operation::And => {
                get_enriched_media_by_logical_and(&tag_ids, limit, repo.as_ref())
            }
            media::Operation::Or => {
                get_enriched_media_by_logical_or(&tag_ids, limit, repo.as_ref())
            }
        }
        .map_err(|error| error.to_string())?;
        let selected_tags = selected_tag_labels(&tag_ids, repo.as_ref())?;
        Ok::<_, String>((query, selected_tags))
    })
    .await;

    match result {
        Ok(Ok((query, selected_tags))) => media::render(
            &selected_tags,
            params.limit,
            params.operation,
            Some(&query),
            None,
            "",
        ),
        Ok(Err(error)) => {
            eprintln!("Error querying media for selected tags: {error}");
            media::render(
                &placeholder_tags,
                params.limit,
                params.operation,
                None,
                Some(&error),
                "The media search could not be completed.",
            )
        }
        Err(error) => {
            eprintln!("Media query task failed: {error}");
            media::render(
                &placeholder_tags,
                params.limit,
                params.operation,
                None,
                Some(&error.to_string()),
                "The media search could not be completed.",
            )
        }
    }
}

struct MediaQueryParams {
    tag_ids: Vec<String>,
    limit: usize,
    operation: media::Operation,
}

fn parse_media_query(query: &str) -> MediaQueryParams {
    let mut tag_ids = Vec::new();
    let mut limit = media::DEFAULT_LIMIT;
    let mut operation = media::Operation::And;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "tag" if !value.is_empty() => {
                let id = value.into_owned();
                if !tag_ids.contains(&id) {
                    tag_ids.push(id);
                }
            }
            "limit" => {
                limit = if value == "unlimited" {
                    usize::MAX
                } else {
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| media::LIMIT_OPTIONS.contains(value))
                        .unwrap_or(media::DEFAULT_LIMIT)
                };
            }
            "operation" => {
                operation = if value == "or" {
                    media::Operation::Or
                } else {
                    media::Operation::And
                };
            }
            _ => {}
        }
    }
    MediaQueryParams {
        tag_ids,
        limit,
        operation,
    }
}

struct MediaPickerParams {
    prefix: String,
    relationship: String,
    selected_ids: Vec<String>,
    limit: usize,
}

fn parse_media_picker_query(query: &str) -> MediaPickerParams {
    let mut params = MediaPickerParams {
        prefix: String::new(),
        relationship: String::new(),
        selected_ids: Vec::new(),
        limit: media::DEFAULT_LIMIT,
    };
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "prefix" => params.prefix = value.into_owned(),
            "query" => params.prefix = value.into_owned(),
            "name" => params.relationship = value.into_owned(),
            "selected" => {
                for id in value.split(',').filter(|id| !id.is_empty()) {
                    if !params.selected_ids.iter().any(|selected| selected == id) {
                        params.selected_ids.push(id.to_string());
                    }
                }
            }
            "limit" => {
                params.limit = if value == "unlimited" {
                    usize::MAX
                } else {
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| media::LIMIT_OPTIONS.contains(value))
                        .unwrap_or(media::DEFAULT_LIMIT)
                };
            }
            _ => {}
        }
    }
    params
}

fn selected_tag_labels<R>(tag_ids: &[String], repo: &R) -> Result<Vec<Tag>, String>
where
    R: MediaRepository + TagRepository,
    <R as TagRepository>::Error: std::fmt::Display,
{
    let mut selected_tags = Vec::with_capacity(tag_ids.len());
    for tag_id in tag_ids {
        let mut name = None;
        let relationships = get_tag_adj_list(tag_id, repo)
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        for (relationship, _) in relationships {
            let tags = get_tags_by_relationship(&relationship, repo)
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            if let Some(tag) = tags.into_iter().find(|tag| tag.id() == tag_id) {
                name = Some(tag.name().to_string());
                break;
            }
        }
        selected_tags.push(Tag::from_parts(
            tag_id.clone(),
            name.unwrap_or_else(|| format!("Unknown tag {}", short_id(tag_id))),
        ));
    }
    Ok(selected_tags)
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn patch_response(
    selector: &'static str,
    markup: maud::Markup,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let event = PatchElements::new(markup.into_string())
        .selector(selector)
        .mode(ElementPatchMode::Outer)
        .into();
    Sse::new(iter([Ok(event)]))
}

async fn search_relationships(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let prefix = params.get("prefix").cloned().unwrap_or_default();
    let repo = state.repo;
    let query_prefix = prefix.clone();
    let result = tokio::task::spawn_blocking(move || {
        get_relationships_by_prefix(&query_prefix, Some(12), repo.as_ref())
    })
    .await;

    let markup = match result {
        Ok(Ok(relationships)) => {
            explore::suggestions(&prefix, relationships.as_deref().unwrap_or_default())
        }
        Ok(Err(error)) => {
            eprintln!("Error searching relationships by prefix {prefix:?}: {error}");
            explore::suggestion_error(&format!("Could not search relationships: {error}"))
        }
        Err(error) => {
            eprintln!("Relationship search task failed for prefix {prefix:?}: {error}");
            explore::suggestion_error(&format!("Relationship search failed: {error}"))
        }
    };

    patch_response("#relationship-suggestions", markup)
}

async fn show_relationship(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let relationship = params.get("name").cloned().unwrap_or_default();
    let markup = relationship_tags_markup(state.repo, relationship).await;
    patch_response("#relationship-details", markup)
}

async fn media_picker_search(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let params = parse_media_picker_query(raw_query.as_deref().unwrap_or_default());
    let prefix = params.prefix.clone();
    let query_prefix = prefix.clone();
    let repo = state.repo;
    let result = tokio::task::spawn_blocking(move || {
        get_relationships_by_prefix(&query_prefix, Some(12), repo.as_ref())
    })
    .await;

    let markup = match result {
        Ok(Ok(relationships)) => media::picker_suggestions(
            &prefix,
            relationships.as_deref().unwrap_or_default(),
            &params.selected_ids,
            params.limit,
        ),
        Ok(Err(error)) => {
            eprintln!("Error searching media-picker relationships by prefix {prefix:?}: {error}");
            media::picker_suggestion_error(&format!("Could not search relationships: {error}"))
        }
        Err(error) => {
            eprintln!("Media-picker relationship search task failed for {prefix:?}: {error}");
            media::picker_suggestion_error(&format!("Relationship search failed: {error}"))
        }
    };
    patch_response("#media-relationship-suggestions", markup)
}

async fn media_picker_relationship(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let params = parse_media_picker_query(raw_query.as_deref().unwrap_or_default());
    let markup = load_media_picker_relationship(
        state.repo,
        params.relationship,
        params.selected_ids,
        params.limit,
    )
    .await;
    patch_response("#media-relationship-details", markup)
}

async fn media_picker_submit_relationship(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let params = parse_media_picker_query(raw_query.as_deref().unwrap_or_default());
    let query = params.prefix.clone();
    if normalize_relationship(&query).is_empty() {
        return patch_response(
            "#media-relationship-details",
            media::picker_message("Type a relationship to see its tags."),
        );
    }

    let repo = state.repo;
    let normalized_query = normalize_relationship(&query);
    let query_for_task = query.clone();
    let result = tokio::task::spawn_blocking(move || {
        let relationships = get_relationships_by_prefix(&query_for_task, None, repo.as_ref())
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        let selected = relationships
            .iter()
            .find(|relationship| **relationship == normalized_query)
            .or_else(|| relationships.first());
        match selected {
            Some(relationship) => {
                let tags = get_tags_by_relationship(relationship, repo.as_ref())
                    .map_err(|error| error.to_string())?
                    .unwrap_or_default();
                Ok::<_, String>(Some((relationship.clone(), tags)))
            }
            None => Ok(None),
        }
    })
    .await;

    let markup = match result {
        Ok(Ok(Some((relationship, tags)))) => media::picker_relationship_tags(
            &relationship,
            &tags,
            &params.selected_ids,
            params.limit,
        ),
        Ok(Ok(None)) => media::picker_message("No matching relationships were found."),
        Ok(Err(error)) => {
            eprintln!("Error resolving media-picker relationship {query:?}: {error}");
            media::picker_relationship_error(&format!("Could not load relationship tags: {error}"))
        }
        Err(error) => {
            eprintln!("Media-picker relationship selection task failed for {query:?}: {error}");
            media::picker_relationship_error(&format!("Relationship search failed: {error}"))
        }
    };
    patch_response("#media-relationship-details", markup)
}

async fn load_media_picker_relationship(
    repo: Arc<RedbMediaRepository>,
    relationship: String,
    selected_ids: Vec<String>,
    limit: usize,
) -> maud::Markup {
    if normalize_relationship(&relationship).is_empty() {
        return media::picker_message("Choose a relationship to see its tags.");
    }

    match tokio::task::spawn_blocking(move || {
        get_tags_by_relationship(&relationship, repo.as_ref())
            .map(|tags| tags.map(|tags| (relationship, tags)))
    })
    .await
    {
        Ok(Ok(Some((relationship, tags)))) => {
            media::picker_relationship_tags(&relationship, &tags, &selected_ids, limit)
        }
        Ok(Ok(None)) => media::picker_message("No tags were found for that relationship."),
        Ok(Err(error)) => {
            eprintln!("Error loading media-picker relationship tags: {error}");
            media::picker_relationship_error(&format!("Could not load relationship tags: {error}"))
        }
        Err(error) => {
            eprintln!("Media-picker relationship tag lookup task failed: {error}");
            media::picker_relationship_error(&format!("Relationship tag lookup failed: {error}"))
        }
    }
}

async fn submit_relationship_search(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let query = params.get("query").cloned().unwrap_or_default();
    if normalize_relationship(&query).is_empty() {
        return patch_response(
            "#relationship-details",
            explore::selection_message("Type a relationship to see its tags."),
        );
    }

    let repo = state.repo;
    let query_for_task = query.clone();
    let result = tokio::task::spawn_blocking(move || {
        let relationships = get_relationships_by_prefix(&query_for_task, None, repo.as_ref())
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        let normalized_query = normalize_relationship(&query_for_task);
        let selected = relationships
            .iter()
            .find(|relationship| **relationship == normalized_query)
            .or_else(|| relationships.first());

        match selected {
            Some(relationship) => {
                let tags = get_tags_by_relationship(relationship, repo.as_ref())
                    .map_err(|error| error.to_string())?
                    .unwrap_or_default();
                Ok::<_, String>(Some((relationship.clone(), tags)))
            }
            None => Ok::<_, String>(None),
        }
    })
    .await;

    let markup = match result {
        Ok(Ok(Some((relationship, tags)))) => explore::relationship_tags(&relationship, &tags),
        Ok(Ok(None)) => explore::selection_message("No matching relationships were found."),
        Ok(Err(error)) => {
            eprintln!("Error resolving relationship search {query:?}: {error}");
            explore::relationship_error(&format!("Could not load relationship tags: {error}"))
        }
        Err(error) => {
            eprintln!("Relationship selection task failed for query {query:?}: {error}");
            explore::relationship_error(&format!("Relationship search failed: {error}"))
        }
    };

    patch_response("#relationship-details", markup)
}

async fn relationship_tags_markup(
    repo: Arc<RedbMediaRepository>,
    relationship: String,
) -> maud::Markup {
    if normalize_relationship(&relationship).is_empty() {
        return explore::selection_message("Choose a relationship to see its tags.");
    }

    match tokio::task::spawn_blocking(move || {
        get_tags_by_relationship(&relationship, repo.as_ref())
            .map(|tags| tags.map(|tags| (relationship, tags)))
    })
    .await
    {
        Ok(Ok(Some((relationship, tags)))) => explore::relationship_tags(&relationship, &tags),
        Ok(Ok(None)) => explore::selection_message("No tags were found for that relationship."),
        Ok(Err(error)) => {
            eprintln!("Error loading tags for relationship: {error}");
            explore::relationship_error(&format!("Could not load relationship tags: {error}"))
        }
        Err(error) => {
            eprintln!("Relationship tag lookup task failed: {error}");
            explore::relationship_error(&format!("Relationship tag lookup failed: {error}"))
        }
    }
}

/// Prepends `markup` to the processed-media feed.
fn feed_event(markup: String) -> axum::response::sse::Event {
    PatchElements::new(markup)
        .selector("#processed-feed")
        .mode(ElementPatchMode::Prepend)
        .into()
}

/// Progress signal update: `processedCount` always advances per field outcome,
/// and `errored` flips to true if that outcome was a failure.
fn progress_event(processed_count: usize, errored: bool) -> axum::response::sse::Event {
    let flag = if errored { ", errored: true" } else { "" };
    PatchSignals::new(format!("{{processedCount: {processed_count}{flag}}}")).into()
}

enum StagedUpload {
    File {
        filename: String,
        content_type: Option<String>,
        file_size: u64,
        temp_file: tempfile::NamedTempFile,
    },
    Failed {
        filename: String,
        reason: String,
    },
}

fn parse_upload_tag_pair(value: &str) -> Result<(String, String), String> {
    let mut relationship = None;
    let mut tag_name = None;
    for (key, value) in url::form_urlencoded::parse(value.as_bytes()) {
        match key.as_ref() {
            "relationship" if relationship.is_none() => {
                relationship = Some(value.into_owned());
            }
            "tag" if tag_name.is_none() => {
                tag_name = Some(value.into_owned());
            }
            "relationship" | "tag" => return Err("A tag pair contains duplicate fields.".into()),
            _ => return Err("A tag pair contains an unsupported field.".into()),
        }
    }

    let relationship = relationship.ok_or("Each selected tag needs a relationship.")?;
    let relationship = recondo_domain::model::new_relationship(&relationship)
        .map_err(|error| error.to_string())?;
    let tag = Tag::new(tag_name.ok_or("Each selected relationship needs a tag.")?)
        .map_err(|error| error.to_string())?;
    Ok((relationship, tag.name().to_string()))
}

async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let response_stream = stream! {
        // The feed placeholder is removed client-side when the upload starts
        // (see the datastar-fetch 'started' handler on the upload page);
        // sending a replace patch here would warn about a missing selector on
        // every upload after the first.

        let mut staged_uploads = Vec::new();
        let mut tag_pairs = Vec::new();
        let mut request_error = None;
        loop {
            let mut field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(err) => {
                    request_error = Some(format!("Upload stream error: {err}"));
                    break;
                }
            };

            let field_name = field.name().unwrap_or_default().to_string();
            let filename = field.file_name().map(str::to_string);
            let content_type = field.content_type().map(str::to_string);

            if field_name == "tag_pair" && filename.is_none() {
                match field.text().await {
                    Ok(value) => match parse_upload_tag_pair(&value) {
                        Ok(pair) => {
                            if !tag_pairs.contains(&pair) {
                                tag_pairs.push(pair);
                            }
                        }
                        Err(error) => request_error = Some(error),
                    },
                    Err(error) => request_error = Some(format!("Could not read selected tags: {error}")),
                }
                continue;
            }

            if field_name != "files" || filename.is_none() {
                let mut drain_error = None;
                while let Some(chunk) = match field.chunk().await {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        drain_error = Some(error.to_string());
                        None
                    }
                } {
                    let _ = chunk;
                }
                request_error.get_or_insert_with(|| {
                    format!("Unexpected multipart field {field_name:?}.")
                });
                if let Some(error) = drain_error {
                    request_error.get_or_insert_with(|| format!("Upload stream error: {error}"));
                    break;
                }
                continue;
            }
            let filename = filename.unwrap_or_else(|| "unnamed_file".to_string());

            // Stage the upload in a temp file, preserving the original extension.
            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|s| s.to_str());

            let mut builder = tempfile::Builder::new();
            let suffix = ext.map(|ext| format!(".{ext}"));
            if let Some(suffix) = &suffix {
                builder.suffix(suffix);
            }

            let mut temp_file = match builder.tempfile() {
                Ok(file) => file,
                Err(err) => {
                    eprintln!("Error creating temp file for {filename}: {err}");
                    loop {
                        match field.chunk().await {
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(error) => {
                                request_error = Some(format!("Upload stream error: {error}"));
                                break;
                            }
                        }
                    }
                    staged_uploads.push(StagedUpload::Failed {
                        filename,
                        reason: format!("Could not stage file: {err}"),
                    });
                    continue;
                }
            };

            // Stream chunks straight to disk; the whole file never sits in memory.
            let mut file_size = 0u64;
            let mut read_error = None;
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if let Err(err) = temp_file.write_all(&chunk) {
                            read_error = Some(format!("Could not write file: {err}"));
                            break;
                        }
                        file_size += chunk.len() as u64;
                    }
                    Ok(None) => break,
                    Err(err) => {
                        read_error = Some(format!("Upload stream error: {err}"));
                        break;
                    }
                }
            }
            if let Some(reason) = read_error {
                eprintln!("Error staging {filename}: {reason}");
                staged_uploads.push(StagedUpload::Failed {
                    filename,
                    reason,
                });
                continue;
            }
            staged_uploads.push(StagedUpload::File {
                filename,
                content_type,
                file_size,
                temp_file,
            });
        }

        if let Some(error) = request_error {
            eprintln!("Rejected malformed upload request: {error}");
            yield Ok(feed_event(upload::media_error_card("(upload)", &error).into_string()));
            yield Ok(PatchSignals::new("{errored: true}").into());
        } else {
            let mut processed_count = 0usize;
            for staged in staged_uploads {
            let (card, failed) = match staged {
                StagedUpload::Failed { filename, reason } => {
                    (upload::media_error_card(&filename, &reason), true)
                }
                StagedUpload::File {
                    filename,
                    content_type,
                    file_size,
                    temp_file,
                } => {
                    let temp_path = temp_file.path().to_path_buf();
                    let repo = state.repo.clone();
                    let dest_root = state.assets_dir.clone();
                    let selected_tags = tag_pairs.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        let result = create_media_from_file_in_with_tags(
                            temp_path,
                            dest_root.as_path(),
                            &selected_tags,
                            repo.as_ref(),
                        );
                        drop(temp_file);
                        result
                    })
                    .await;

                    match result {
                        Ok(Ok(CreateMediaOutcome::Created(media))) => (
                            upload::media_processed_card(
                                &filename,
                                media.id.as_str(),
                                "PROCESSED",
                                file_size,
                                content_type.as_deref().or(media.mime_type.as_deref()),
                                false,
                                &tag_pairs,
                            ),
                            false,
                        ),
                        Ok(Ok(CreateMediaOutcome::Duplicate(id))) => (
                            upload::media_processed_card(
                                &filename,
                                id.as_str(),
                                "DUPLICATE",
                                file_size,
                                content_type.as_deref(),
                                true,
                                &tag_pairs,
                            ),
                            false,
                        ),
                        Ok(Err(err)) => {
                            eprintln!("Error creating media from file {filename}: {err}");
                            (upload::media_error_card(&filename, &err.to_string()), true)
                        }
                        Err(join_err) => {
                            eprintln!("Join error while creating media: {join_err}");
                            (
                                upload::media_error_card(
                                    &filename,
                                    &format!("Processing failed: {join_err}"),
                                ),
                                true,
                            )
                        }
                    }
                }
            };

                yield Ok(feed_event(card.into_string()));
                processed_count += 1;
                yield Ok(progress_event(processed_count, failed));
            }
        }
    };

    Sse::new(response_stream)
}

async fn filter_demo(
    Path(filter): Path<String>,
) -> Result<
    Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>>,
    StatusCode,
> {
    let filter = DemoFilter::from_slug(&filter).ok_or(StatusCode::NOT_FOUND)?;
    let elements = maud::html! {
        (landing::filter_controls(filter))
        (landing::results(filter))
    }
    .into_string();
    let event = PatchElements::new(elements)
        .mode(ElementPatchMode::Outer)
        .into();

    Ok(Sse::new(iter([Ok(event)])))
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/styles.css"),
    )
}

async fn media_asset(Path(filename): Path<String>) -> Response {
    let bytes = match filename.as_str() {
        "data_trap.png" => &include_bytes!("../tests/fixtures/data_trap.png")[..],
        "spreadsheets.png" => &include_bytes!("../tests/fixtures/spreadsheets.png")[..],
        "excel_lambda_new.png" => &include_bytes!("../tests/fixtures/excel_lambda_new.png")[..],
        "exploits_of_a_mom.png" => &include_bytes!("../tests/fixtures/exploits_of_a_mom.png")[..],
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    (
        [(header::CONTENT_TYPE, "image/png")],
        Body::from(bytes.to_vec()),
    )
        .into_response()
}

async fn media_asset_by_id(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Ok(media_id) = MediaId::from_hash(id.clone()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let repo = state.repo;
    let media = match tokio::task::spawn_blocking(move || repo.get_media(&media_id)).await {
        Ok(Ok(Some(media))) => media,
        Ok(Ok(None)) => return StatusCode::NOT_FOUND.into_response(),
        Ok(Err(error)) => {
            eprintln!("Could not find media asset {id}: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(error) => {
            eprintln!("Media asset lookup task failed for {id}: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let assets_dir = match tokio::fs::canonicalize(&state.assets_dir).await {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(error) => {
            eprintln!("Could not resolve assets directory: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let media_filename = media
        .url
        .to_file_path()
        .ok()
        .and_then(|path| path.file_name().map(std::ffi::OsStr::to_os_string));
    let Some(media_filename) = media_filename else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let media_path = match tokio::fs::canonicalize(assets_dir.join(media_filename)).await {
        Ok(path)
            if path.starts_with(&assets_dir)
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem == id) =>
        {
            path
        }
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(error) => {
            eprintln!("Could not resolve media asset {id}: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut file = match tokio::fs::File::open(media_path).await {
        Ok(file) => file,
        Err(error) => {
            eprintln!("Could not open media asset {id}: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let file_size = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            eprintln!("Could not inspect media asset {id}: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let requested_range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let range = match requested_range {
        Some(value) => match parse_byte_range(value, file_size) {
            Ok(range) => range,
            Err(()) => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{file_size}"))
                    .body(Body::empty())
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
        },
        None => None,
    };

    let (start, content_length, status) = match range {
        Some((start, end)) => (start, end - start + 1, StatusCode::PARTIAL_CONTENT),
        None => (0, file_size, StatusCode::OK),
    };
    if start > 0
        && let Err(error) = file.seek(SeekFrom::Start(start)).await
    {
        eprintln!("Could not seek media asset {id}: {error}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let content_type = media
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, content_length);
    if let Some((start, end)) = range {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_size}"),
        );
    }

    let body_stream = stream! {
        let mut remaining = content_length;
        let mut buffer = vec![0u8; 64 * 1024];
        while remaining > 0 {
            let chunk_size = remaining.min(buffer.len() as u64) as usize;
            let bytes_read = match file.read(&mut buffer[..chunk_size]).await {
                Ok(bytes_read) => bytes_read,
                Err(error) => {
                    yield Err::<Bytes, std::io::Error>(error);
                    break;
                }
            };
            if bytes_read == 0 {
                break;
            }
            remaining -= bytes_read as u64;
            yield Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(&buffer[..bytes_read]));
        }
    };
    response
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|error| {
            eprintln!("Could not build media response for {id}: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

fn parse_byte_range(value: &str, file_size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(value) = value.strip_prefix("bytes=") else {
        return Ok(None);
    };
    if value.contains(',') {
        return Ok(None);
    }
    let Some((start, end)) = value.split_once('-') else {
        return Err(());
    };

    if start.is_empty() {
        let suffix_length = end.parse::<u64>().map_err(|_| ())?;
        if suffix_length == 0 || file_size == 0 {
            return Err(());
        }
        return Ok(Some((
            file_size.saturating_sub(suffix_length),
            file_size - 1,
        )));
    }

    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= file_size {
        return Err(());
    }
    let end = if end.is_empty() {
        file_size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(file_size - 1)
    };
    if end < start {
        return Err(());
    }
    Ok(Some((start, end)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use recondo_dal::media::RedbMediaRepository;
    use recondo_domain::cmd::{CreateMediaOutcome, add_tag_to_media, create_media_from_file_in};
    use recondo_domain::model::{MediaId, Tag};
    use recondo_domain::repository::MediaRepository;
    use redb::Database;
    use tower::ServiceExt;

    use super::{AppState, parse_upload_tag_pair, router};

    /// Builds an `AppState` backed by a single temp dir (redb database +
    /// `assets/` subdir). The `TempDir` is returned so tests can keep it
    /// alive for their full lifetime — nothing is ever written into the
    /// repository tree.
    fn test_app_state() -> (AppState, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let db = Database::create(&db_path).unwrap();
        let repo = RedbMediaRepository::new(db);
        repo.init().unwrap();
        let state = AppState {
            repo: Arc::new(repo),
            assets_dir: temp_dir.path().join("assets"),
        };
        (state, temp_dir)
    }

    fn seed_explore_relationships(state: &AppState, temp: &tempfile::TempDir) -> String {
        let source = temp.path().join("explore-seed.txt");
        std::fs::write(&source, "explore relationship seed").unwrap();
        let media = match create_media_from_file_in(
            source,
            &temp.path().join("assets"),
            state.repo.as_ref(),
        )
        .unwrap()
        {
            CreateMediaOutcome::Created(media) => media,
            CreateMediaOutcome::Duplicate(_) => panic!("test media should be unique"),
        };
        let media_id = media.id.as_str().to_string();

        add_tag_to_media(
            media.id.as_str(),
            "field note",
            "to do",
            state.repo.as_ref(),
        )
        .unwrap();
        add_tag_to_media(media.id.as_str(), "topic tag", "topic", state.repo.as_ref()).unwrap();
        media_id
    }

    #[tokio::test]
    async fn landing_page_contains_project_summary_and_demo() {
        let (state, _tmp) = test_app_state();
        let response = router(state)
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("graph-based media discovery"));
        assert!(html.contains("Choose a tag to see how a connection"));
        assert!(html.contains("data_trap.png"));
        assert!(html.contains("datastar.js"));
    }

    #[tokio::test]
    async fn upload_page_renders_dropzone_and_components() {
        let (state, _tmp) = test_app_state();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/upload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("Upload "));
        assert!(html.contains("<em>media files.</em>"));
        assert!(html.contains("upload-dropzone"));
        assert!(html.contains("file-input"));
        assert!(html.contains("progress-bar"));
        // Drop only works when dragover's default is canceled. Datastar v1
        // modifiers use `__`, not a second `:` (which would make the event
        // name "dragover:prevent" — a non-existent DOM event).
        assert!(html.contains("data-on:dragover__prevent"));
        assert!(html.contains("data-on:drop__prevent"));
        assert!(!html.contains("data-on:dragover:prevent"));
        assert!(!html.contains("data-on:drop:prevent"));
        assert!(html.contains("upload-tagging"));
        assert!(html.contains("selected-upload-tags"));
        assert!(html.contains("Add pair"));
        assert!(html.contains("Clear uploaded list"));
        assert!(html.contains("/upload.js"));
        // The placeholder is removed client-side on upload start.
        assert!(html.contains("feed-placeholder"));
    }

    #[test]
    fn upload_tag_pair_parser_requires_a_complete_pair() {
        assert_eq!(
            parse_upload_tag_pair("relationship=to+do&tag=needs+tags").unwrap(),
            ("to do".to_string(), "needs tags".to_string())
        );
        assert!(parse_upload_tag_pair("relationship=to+do").is_err());
        assert!(parse_upload_tag_pair("tag=needs+tags").is_err());
        assert!(parse_upload_tag_pair("relationship=+&tag=+").is_err());
    }

    #[tokio::test]
    async fn upload_relationship_picker_reuses_live_relationship_and_tag_queries() {
        let (state, temp) = test_app_state();
        seed_explore_relationships(&state, &temp);
        let search = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/upload/relationship-search?prefix=to")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let search_body = to_bytes(search.into_body(), usize::MAX).await.unwrap();
        let search_html = String::from_utf8(search_body.to_vec()).unwrap();
        assert!(search_html.contains("to do"));

        let relationship = router(state)
            .oneshot(
                Request::builder()
                    .uri("/upload/relationship?name=to%20do")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let relationship_body = to_bytes(relationship.into_body(), usize::MAX)
            .await
            .unwrap();
        let relationship_html = String::from_utf8(relationship_body.to_vec()).unwrap();
        assert!(relationship_html.contains("needs tags"));
        assert!(relationship_html.contains("window.addUploadTagPair"));
        assert!(relationship_html.contains("data-relationship=\"to do\""));
    }

    #[tokio::test]
    async fn demo_filter_streams_matching_fixture_cards() {
        let (state, _tmp) = test_app_state();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/demo/spreadsheets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let event_stream = String::from_utf8(body.to_vec()).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert!(event_stream.contains("datastar-patch-elements"));
        assert!(event_stream.contains("filter-button is-selected"));
        assert!(event_stream.contains("Excel + Lambda"));
        assert!(!event_stream.contains("Exploits of a Mom"));
    }

    #[tokio::test]
    async fn demo_filter_rejects_unknown_tags() {
        let (state, _tmp) = test_app_state();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/demo/not-a-filter")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn explore_page_renders_live_relationship_search() {
        let (state, _tmp) = test_app_state();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/explore")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("Explore your "));
        assert!(html.contains("data-on:input"));
        assert!(html.contains("/explore/search?prefix="));
        assert!(html.contains("relationship-suggestions"));
        assert!(html.contains("href=\"/explore\""));
    }

    #[tokio::test]
    async fn explore_search_returns_relationships_matching_prefix() {
        let (state, temp) = test_app_state();
        seed_explore_relationships(&state, &temp);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/explore/search?prefix=t")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let event_stream = String::from_utf8(body.to_vec()).unwrap();

        assert!(event_stream.contains("datastar-patch-elements"));
        assert!(event_stream.contains("to do"));
        assert!(event_stream.contains("topic"));
    }

    #[tokio::test]
    async fn explore_search_reports_when_no_relationships_match() {
        let (state, _temp) = test_app_state();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/explore/search?prefix=missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let event_stream = String::from_utf8(body.to_vec()).unwrap();

        assert!(event_stream.contains("No relationships match"));
    }

    #[tokio::test]
    async fn explore_relationship_selection_shows_all_associated_tags() {
        let (state, temp) = test_app_state();
        seed_explore_relationships(&state, &temp);
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/explore/relationship?name=to%20do")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let event_stream = String::from_utf8(body.to_vec()).unwrap();

        assert!(event_stream.contains("relationship-tag-grid"));
        assert!(event_stream.contains("needs tags"));
        assert!(event_stream.contains("field note"));
        assert!(event_stream.contains("2 tags"));
    }

    #[tokio::test]
    async fn explore_enter_selection_prefers_exact_then_first_prefix_match() {
        let (state, temp) = test_app_state();
        seed_explore_relationships(&state, &temp);
        let exact_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/explore/submit?query=topic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let exact_body = to_bytes(exact_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let exact_event = String::from_utf8(exact_body.to_vec()).unwrap();
        assert!(exact_event.contains("<h2>topic</h2>"));
        assert!(exact_event.contains("topic tag"));
        assert!(!exact_event.contains("needs tags"));

        let prefix_response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/explore/submit?query=t")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let prefix_body = to_bytes(prefix_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let prefix_event = String::from_utf8(prefix_body.to_vec()).unwrap();
        assert!(prefix_event.contains("<h2>to do</h2>"));
        assert!(prefix_event.contains("needs tags"));
        assert!(prefix_event.contains("field note"));
    }

    #[tokio::test]
    async fn media_page_shows_selected_tags_related_groups_and_downloads() {
        let (state, temp) = test_app_state();
        seed_explore_relationships(&state, &temp);
        let tag_id = Tag::new("field note").unwrap().id().to_string();
        let other_tag_id = Tag::new("topic tag").unwrap().id().to_string();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/media?tag={tag_id}&tag={other_tag_id}&limit=25"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();

        assert!(html.contains("field note"));
        assert!(html.contains("topic tag"));
        assert!(html.contains("needs tags"));
        assert!(html.contains("DISPLAY LIMIT"));
        assert!(html.contains("AND then OR and OR then AND are not functional yet."));
        assert!(html.contains(&format!("href=\"/media?tag={other_tag_id}&amp;limit=25\"")));
        assert!(html.contains("Download file"));
        assert!(html.contains("data-on:change"));
    }

    #[tokio::test]
    async fn media_or_query_uses_logical_or_and_picker_excludes_selected_tags() {
        let (state, temp) = test_app_state();
        let first_media_id = seed_explore_relationships(&state, &temp);
        let second_source = temp.path().join("second-media.txt");
        std::fs::write(&second_source, "a distinct media item").unwrap();
        let second_media = match create_media_from_file_in(
            second_source,
            &temp.path().join("assets"),
            state.repo.as_ref(),
        )
        .unwrap()
        {
            CreateMediaOutcome::Created(media) => media,
            CreateMediaOutcome::Duplicate(_) => panic!("test media should be unique"),
        };
        add_tag_to_media(
            second_media.id.as_str(),
            "another subject",
            "subject",
            state.repo.as_ref(),
        )
        .unwrap();

        let first_tag_id = Tag::new("field note").unwrap().id().to_string();
        let second_tag_id = Tag::new("another subject").unwrap().id().to_string();
        let or_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/media?tag={first_tag_id}&tag={second_tag_id}&operation=or&limit=10"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let or_body = to_bytes(or_response.into_body(), usize::MAX).await.unwrap();
        let or_html = String::from_utf8(or_body.to_vec()).unwrap();
        assert_eq!(or_html.matches("data-testid=\"media-card\"").count(), 2);
        assert!(or_html.contains("aria-label=\"Add tags\""));
        assert!(or_html.contains("name=\"operation\" value=\"or\""));
        assert!(or_html.contains("class=\"operation-button is-selected\""));

        let and_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/media?tag={first_tag_id}&tag={second_tag_id}&limit=10"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let and_body = to_bytes(and_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let and_html = String::from_utf8(and_body.to_vec()).unwrap();
        assert!(and_html.contains("No media matches all selected tags."));
        assert!(!and_html.contains("data-testid=\"media-card\""));
        assert!(!and_html.contains(&format!("/asset/{first_media_id}")));

        let needs_tags_id = Tag::new("needs tags").unwrap().id().to_string();
        let picker_response = router(state)
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/media/relationship?name=to%20do&selected={needs_tags_id}&limit=25"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let picker_body = to_bytes(picker_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let picker_html = String::from_utf8(picker_body.to_vec()).unwrap();
        let field_note_id = Tag::new("field note").unwrap().id().to_string();
        assert!(picker_html.contains("field note"));
        assert!(!picker_html.contains("needs tags"));
        assert!(picker_html.contains(&format!(
            "href=\"/media?tag={needs_tags_id}&amp;tag={field_note_id}&amp;limit=25&amp;operation=or\""
        )));
    }

    #[tokio::test]
    async fn media_limit_shows_count_at_limit_and_items_under_limit() {
        let (state, temp) = test_app_state();
        let source = temp.path().join("media-limit.txt");
        let mut first_id = String::new();
        for index in 0..10 {
            std::fs::write(&source, format!("media item {index}")).unwrap();
            let media = match create_media_from_file_in(
                source.clone(),
                &temp.path().join("assets"),
                state.repo.as_ref(),
            )
            .unwrap()
            {
                CreateMediaOutcome::Created(media) => media,
                CreateMediaOutcome::Duplicate(_) => panic!("test media should be unique"),
            };
            if index == 0 {
                first_id = media.id.as_str().to_string();
            }
            add_tag_to_media(
                media.id.as_str(),
                "many matches",
                "subject",
                state.repo.as_ref(),
            )
            .unwrap();
        }
        let tag_id = Tag::new("many matches").unwrap().id().to_string();

        let count_response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/media?tag={tag_id}&limit=10"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let count_body = to_bytes(count_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let count_html = String::from_utf8(count_body.to_vec()).unwrap();
        assert!(count_html.contains("media-count-number"));
        assert!(count_html.contains(">10</span>"));
        assert!(!count_html.contains("data-testid=\"media-card\""));

        let items_response = router(state)
            .oneshot(
                Request::builder()
                    .uri(format!("/media?tag={tag_id}&limit=25"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items_body = to_bytes(items_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let items_html = String::from_utf8(items_body.to_vec()).unwrap();
        assert_eq!(items_html.matches("data-testid=\"media-card\"").count(), 10);
        assert!(items_html.contains(&format!("/asset/{first_id}")));
    }

    #[tokio::test]
    async fn media_asset_route_serves_only_media_stored_inside_assets_directory() {
        let (state, temp) = test_app_state();
        let media_id = seed_explore_relationships(&state, &temp);
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/asset/{media_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"explore relationship seed");

        let partial = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/asset/{media_id}"))
                    .header(header::RANGE, "bytes=0-6")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.headers()[header::CONTENT_RANGE], "bytes 0-6/25");
        let partial_body = to_bytes(partial.into_body(), usize::MAX).await.unwrap();
        assert_eq!(partial_body.as_ref(), b"explore");

        let unsatisfiable = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/asset/{media_id}"))
                    .header(header::RANGE, "bytes=100-")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unsatisfiable.status(), StatusCode::RANGE_NOT_SATISFIABLE);

        let missing = router(state)
            .oneshot(
                Request::builder()
                    .uri("/asset/not-in-the-database")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn fixture_route_serves_only_known_images() {
        let (state, _tmp) = test_app_state();
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/media/data_trap.png")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");

        let missing = router(state)
            .oneshot(
                Request::builder()
                    .uri("/media/not-a-fixture.png")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn upload_handler_processes_files_and_streams_patches() {
        let (state, tmp) = test_app_state();
        let boundary = "---------------------------1234567890";
        let body_content = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"test1.png\"\r\n\
             Content-Type: image/png\r\n\r\n\
             fake_image_content_1\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"test2.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             fake_text_content_2\r\n\
             --{boundary}--\r\n"
        );

        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body_content))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let sse_text = String::from_utf8(body.to_vec()).unwrap();

        assert!(sse_text.contains("datastar-patch-elements"));
        assert!(sse_text.contains("test1.png"));
        assert!(sse_text.contains("test2.txt"));
        assert!(sse_text.contains("PROCESSED"));
        // Progress signals stream once per processed field.
        assert!(sse_text.contains("processedCount: 1"));
        assert!(sse_text.contains("processedCount: 2"));

        // Copies land in the test's temp assets dir, never in the repo tree.
        let copied: Vec<_> = std::fs::read_dir(tmp.path().join("assets"))
            .expect("temp assets dir should exist after upload")
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            copied.len(),
            2,
            "expected a png + txt copy in the temp dir, got {copied:?}"
        );

        // Now upload duplicate file and verify duplicate handling
        let dup_body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"test1.png\"\r\n\
             Content-Type: image/png\r\n\r\n\
             fake_image_content_1\r\n\
             --{boundary}--\r\n"
        );

        let dup_response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(dup_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(dup_response.status(), StatusCode::OK);
        let dup_body = to_bytes(dup_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let dup_text = String::from_utf8(dup_body.to_vec()).unwrap();

        assert!(dup_text.contains("DUPLICATE"));

        // A duplicate must not write another copy into the assets dir.
        let copied_count = std::fs::read_dir(tmp.path().join("assets"))
            .unwrap()
            .count();
        assert_eq!(
            copied_count, 2,
            "duplicate upload must not create an extra file"
        );
    }

    #[tokio::test]
    async fn upload_handler_persists_selected_pairs_for_new_and_duplicate_media() {
        let (state, _temp) = test_app_state();
        let boundary = "---------------------------tagged-upload";
        let content = "media with upload tags";
        let make_body = |relationship: &str, tag: &str| {
            let pair = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("relationship", relationship)
                .append_pair("tag", tag)
                .finish();
            format!(
                "--{boundary}\r\n\
                 Content-Disposition: form-data; name=\"tag_pair\"\r\n\r\n\
                 {pair}\r\n\
                 --{boundary}\r\n\
                 Content-Disposition: form-data; name=\"files\"; filename=\"tagged.txt\"\r\n\
                 Content-Type: text/plain\r\n\r\n\
                 {content}\r\n\
                 --{boundary}--\r\n"
            )
        };
        let upload = |state: AppState, body: String| async move {
            router(state)
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/upload")
                        .header(
                            header::CONTENT_TYPE,
                            format!("multipart/form-data; boundary={boundary}"),
                        )
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap()
        };

        let created_response = upload(state.clone(), make_body("Trip Plan", "Coast Drive")).await;
        let created_body = to_bytes(created_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let created_html = String::from_utf8(created_body.to_vec()).unwrap();
        assert!(created_html.contains("#coast drive · trip plan"));

        let first_tag = Tag::new("coast drive").unwrap();
        let first_tag_edges = recondo_domain::get_tag_adj_list(first_tag.id(), state.repo.as_ref())
            .unwrap()
            .unwrap();
        let media_id = MediaId::from_hash(first_tag_edges[0].1.clone()).unwrap();
        let first_edges = state.repo.get_media_adj_list(&media_id).unwrap().unwrap();
        assert_eq!(
            first_edges,
            vec![("trip plan".to_string(), first_tag.id().to_string())]
        );
        let default_tag = Tag::new("needs tags").unwrap();
        assert!(
            !recondo_domain::get_tag_adj_list(default_tag.id(), state.repo.as_ref())
                .unwrap()
                .unwrap_or_default()
                .iter()
                .any(|(_, id)| id == media_id.as_str())
        );

        let duplicate_response = upload(state.clone(), make_body("occasion", "birthday")).await;
        let duplicate_body = to_bytes(duplicate_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let duplicate_html = String::from_utf8(duplicate_body.to_vec()).unwrap();
        assert!(duplicate_html.contains("DUPLICATE"));

        let second_tag = Tag::new("birthday").unwrap();
        let updated_edges = state.repo.get_media_adj_list(&media_id).unwrap().unwrap();
        assert_eq!(
            updated_edges,
            vec![
                ("trip plan".to_string(), first_tag.id().to_string()),
                ("occasion".to_string(), second_tag.id().to_string()),
            ]
        );
        assert!(
            !recondo_domain::get_tag_adj_list(default_tag.id(), state.repo.as_ref())
                .unwrap()
                .unwrap_or_default()
                .iter()
                .any(|(_, id)| id == media_id.as_str())
        );
    }

    #[tokio::test]
    async fn upload_handler_accepts_bodies_over_default_2mb_limit() {
        let (state, tmp) = test_app_state();
        let boundary = "---------------------------bigupload";
        let big = vec![7u8; 3 * 1024 * 1024];

        let mut body_content = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"big.bin\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        body_content.extend_from_slice(&big);
        body_content.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body_content))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let sse_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            sse_text.contains("PROCESSED"),
            "3MB upload should stream a processed card, got:\n{sse_text}"
        );
        assert_eq!(
            std::fs::read_dir(tmp.path().join("assets"))
                .expect("temp assets dir should exist")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn upload_handler_surfaces_truncated_stream_as_failed_card() {
        let (state, _tmp) = test_app_state();
        let boundary = "---------------------------truncated";
        // Field header + partial data, but the closing boundary is never sent:
        // the multipart parser hits unexpected end-of-stream mid-field.
        let body_content = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"files\"; filename=\"broken.png\"\r\n\
             Content-Type: image/png\r\n\r\n\
             partial image bytes"
        );

        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body_content))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let sse_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            sse_text.contains("FAILED"),
            "truncated upload must surface an error card, got:\n{sse_text}"
        );
    }

    use axum::body::Body;
    use axum::http::header;
}
