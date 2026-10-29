use std::convert::Infallible;

use axum::{
    Router,
    body::Body,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response, Sse},
    routing::get,
};
use datastar::{consts::ElementPatchMode, prelude::PatchElements};
use tokio_stream::iter;

use crate::pages::landing::{self, DemoFilter};

pub fn router() -> Router {
    Router::new()
        .route("/", get(landing_page))
        .route("/demo/{filter}", get(filter_demo))
        .route("/styles.css", get(stylesheet))
        .route("/media/{filename}", get(media_asset))
}

async fn landing_page() -> maud::Markup {
    landing::render()
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
    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::router;

    #[tokio::test]
    async fn landing_page_contains_project_summary_and_demo() {
        let response = router()
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
    async fn demo_filter_streams_matching_fixture_cards() {
        let response = router()
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
        let response = router()
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
        let response = router()
            .clone()
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

        let missing = router()
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

    use axum::body::Body;
    use axum::http::header;
}
