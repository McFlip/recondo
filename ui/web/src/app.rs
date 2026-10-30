use std::convert::Infallible;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use async_stream::stream;
use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
};
use datastar::{
    consts::ElementPatchMode,
    prelude::{PatchElements, PatchSignals},
};
use recondo_dal::media::RedbMediaRepository;
use recondo_domain::cmd::{CreateMediaOutcome, create_media_from_file_in};
use tokio_stream::iter;

use crate::pages::landing::{self, DemoFilter};
use crate::pages::upload;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<RedbMediaRepository>,
    pub assets_dir: PathBuf,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(landing_page))
        .route("/upload", get(upload_page))
        .route("/upload", post(upload_handler))
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

async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let response_stream = stream! {
        // The feed placeholder is removed client-side when the upload starts
        // (see the datastar-fetch 'started' handler on the upload page);
        // sending a replace patch here would warn about a missing selector on
        // every upload after the first.

        let mut processed_count = 0usize;

        loop {
            let mut field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(err) => {
                    eprintln!("Error reading multipart stream: {err}");
                    yield Ok(feed_event(
                        upload::media_error_card("(upload)", &format!("Upload stream error: {err}"))
                            .into_string(),
                    ));
                    yield Ok(PatchSignals::new("{errored: true}").into());
                    break;
                }
            };

            let filename = field
                .file_name()
                .map(str::to_string)
                .unwrap_or_else(|| "unnamed_file".to_string());
            let content_type = field.content_type().map(str::to_string);

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
                    yield Ok(feed_event(
                        upload::media_error_card(&filename, &format!("Could not stage file: {err}"))
                            .into_string(),
                    ));
                    processed_count += 1;
                    yield Ok(progress_event(processed_count, true));
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
                yield Ok(feed_event(upload::media_error_card(&filename, &reason).into_string()));
                processed_count += 1;
                yield Ok(progress_event(processed_count, true));
                continue;
            }

            let temp_path = temp_file.path().to_path_buf();
            let repo = state.repo.clone();
            let dest_root = state.assets_dir.clone();

            let result = tokio::task::spawn_blocking(move || {
                create_media_from_file_in(temp_path, dest_root.as_path(), repo.as_ref())
            })
            .await;

            let (card, failed) = match result {
                Ok(Ok(CreateMediaOutcome::Created(media))) => (
                    upload::media_processed_card(
                        &filename,
                        media.id.as_str(),
                        "PROCESSED",
                        file_size,
                        content_type.as_deref().or(media.mime_type.as_deref()),
                        false,
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
                        upload::media_error_card(&filename, &format!("Processing failed: {join_err}")),
                        true,
                    )
                }
            };

            yield Ok(feed_event(card.into_string()));

            // Progress: one count per field outcome (created, duplicate, or failed).
            processed_count += 1;
            yield Ok(progress_event(processed_count, failed));
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use recondo_dal::media::RedbMediaRepository;
    use recondo_domain::repository::MediaRepository;
    use redb::Database;
    use tower::ServiceExt;

    use super::{AppState, router};

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
        // The placeholder is removed client-side on upload start.
        assert!(html.contains("feed-placeholder"));
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
