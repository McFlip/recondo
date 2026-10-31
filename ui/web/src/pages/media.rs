use maud::{Markup, html};
use recondo_domain::model::{Media, Tag};
use recondo_domain::{CountedTag, MediaQueryItems, MediaQueryResult, RelationshipTagGroup};

use crate::layout;

pub const DEFAULT_LIMIT: usize = 10;
pub const LIMIT_OPTIONS: [usize; 8] = [10, 25, 50, 75, 100, 150, 200, 400];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Operation {
    #[default]
    And,
    Or,
}

pub fn render(
    selected_tags: &[Tag],
    limit: usize,
    operation: Operation,
    result: Option<&MediaQueryResult>,
    error: Option<&str>,
    empty_message: &str,
) -> Markup {
    let tag_ids = selected_tags
        .iter()
        .map(|tag| tag.id().to_string())
        .collect::<Vec<_>>();
    layout::render(
        "Media results",
        html! {
            section class="shell media-shell section-space" {
                div class="section-heading media-heading" {
                    p class="eyebrow" { "Follow the thread" }
                    h1 { "Your " em { "media." } }
                    p class="section-description" {
                        "Combine tags and discover the media that connects them."
                    }
                }

                div class="media-workspace" {
                    aside
                        class="media-sidebar"
                        aria-label=(if operation == Operation::Or { "Add tags" } else { "Related tags" }) {
                        div class="media-sidebar-heading" {
                            p class="eyebrow" {
                                @if operation == Operation::Or { "BUILD YOUR OR SEARCH" } @else { "DISCOVER" }
                            }
                            h2 {
                                @if operation == Operation::Or { "Add tags" } @else { "Related tags" }
                            }
                        }
                        @if operation == Operation::Or {
                            (relationship_picker(&tag_ids, limit))
                        } @else {
                            nav class="relationship-accordion-list" aria-label="Tags grouped by relationship" {
                                @if let Some(result) = result {
                                    @for group in &result.relationships {
                                        (relationship_group(group, &tag_ids, limit, operation))
                                    }
                                }
                                @if result.is_none_or(|result| result.relationships.is_empty()) {
                                    p class="media-muted" {
                                        @if selected_tags.is_empty() {
                                            "Choose tags to see related connections."
                                        } @else {
                                            "No related tags were found."
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div class="media-main" {
                        nav class="media-toolbar" aria-label="Media search controls" {
                            section class="selected-tags-section" aria-label="Selected tags" {
                                p class="media-control-label" { "SELECTED TAGS" }
                                @if selected_tags.is_empty() {
                                    p class="media-muted" { "No tags selected" }
                                } @else {
                                    div class="selected-tag-list" {
                                        @for tag in selected_tags {
                                            (selected_tag(tag, &tag_ids, limit, operation))
                                        }
                                    }
                                }
                            }

                            section class="media-controls-section" aria-label="Search options" {
                                div class="operation-control" {
                                    p class="media-control-label" { "LOGICAL OPERATION" }
                                    div class="operation-buttons" {
                                        (operation_button("AND", Operation::And, operation, &tag_ids, limit))
                                        (operation_button("OR", Operation::Or, operation, &tag_ids, limit))
                                        button type="button" class="operation-button" disabled { "AND then OR" }
                                        button type="button" class="operation-button" disabled { "OR then AND" }
                                    }
                                    p class="media-control-note" {
                                        "AND then OR and OR then AND are not functional yet."
                                    }
                                }
                                form class="limit-control" method="get" action="/media" {
                                    @for id in &tag_ids {
                                        input type="hidden" name="tag" value=(id);
                                    }
                                    (operation_input(operation))
                                    label for="display-limit" class="media-control-label" { "DISPLAY LIMIT" }
                                    select
                                        id="display-limit"
                                        name="limit"
                                        data-on:change="el.form.requestSubmit()" {
                                        option
                                            value=(DEFAULT_LIMIT)
                                            selected[limit == DEFAULT_LIMIT] {
                                            (DEFAULT_LIMIT)
                                        }
                                        @for option in LIMIT_OPTIONS.into_iter().skip(1) {
                                            option value=(option) selected[limit == option] { (option) }
                                        }
                                        option value="unlimited" selected[limit == usize::MAX] { "Unlimited" }
                                    }
                                    p class="media-control-note" {
                                        "Media is shown only when the number of results is under this limit."
                                    }
                                }
                            }
                        }

                        section class="media-results" aria-live="polite" {
                            @if let Some(error) = error {
                                p class="media-error" { "Media search failed: " (error) }
                            } @else if let Some(result) = result {
                                (query_results(&result.media, operation))
                            } @else {
                                p class="media-empty-state" { (empty_message) }
                            }
                        }
                    }
                }
            }

            dialog
                id="media-lightbox"
                class="media-lightbox"
                aria-label="Full-size image"
                data-on:click="if (evt.target === el) el.close()" {
                button
                    type="button"
                    class="media-lightbox-close"
                    aria-label="Close image"
                    data-on:click="el.closest('dialog').close()" {
                    "×"
                }
                button
                    type="button"
                    id="media-lightbox-prev"
                    class="media-lightbox-nav media-lightbox-prev"
                    aria-label="Previous image"
                    data-on:click="window.mediaLightboxPrev()" {
                    "‹"
                }
                button
                    type="button"
                    id="media-lightbox-next"
                    class="media-lightbox-nav media-lightbox-next"
                    aria-label="Next image"
                    data-on:click="window.mediaLightboxNext()" {
                    "›"
                }
                img id="media-lightbox-image" alt="Full-size media preview";
                a id="media-lightbox-open" class="media-lightbox-open" aria-label="Open details" href="#" { "Open" }
            }
            script src="/media.js" {}
        },
    )
}

fn relationship_group(
    group: &RelationshipTagGroup,
    selected_ids: &[String],
    limit: usize,
    operation: Operation,
) -> Markup {
    html! {
        details class="relationship-accordion" {
            summary {
                span { (group.relationship) }
                span class="relationship-accordion-count" { (group.tags.len()) }
            }
            div class="relationship-accordion-tags" {
                @for counted_tag in &group.tags {
                    (related_tag(counted_tag, selected_ids, limit, operation))
                }
            }
        }
    }
}

fn related_tag(
    counted_tag: &CountedTag,
    selected_ids: &[String],
    limit: usize,
    operation: Operation,
) -> Markup {
    let href = media_url(
        &selected_ids
            .iter()
            .cloned()
            .chain(std::iter::once(counted_tag.tag.id().to_string()))
            .collect::<Vec<_>>(),
        limit,
        operation,
    );
    html! {
        a class="related-tag-link" href=(href) {
            span { (counted_tag.tag.name()) }
            span class="related-tag-count" { (counted_tag.count) }
        }
    }
}

fn selected_tag(tag: &Tag, selected_ids: &[String], limit: usize, operation: Operation) -> Markup {
    let remaining = selected_ids
        .iter()
        .filter(|id| id.as_str() != tag.id())
        .cloned()
        .collect::<Vec<_>>();
    let href = if remaining.is_empty() {
        "/explore".to_string()
    } else {
        media_url(&remaining, limit, operation)
    };
    html! {
        span class="selected-tag-chip" {
            span { (tag.name()) }
            a
                class="selected-tag-remove"
                href=(href)
                aria-label=(format!("Remove {}", tag.name()))
                title=(format!("Remove {}", tag.name())) {
                "×"
            }
        }
    }
}

fn media_url(tag_ids: &[String], limit: usize, operation: Operation) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for tag_id in tag_ids {
        serializer.append_pair("tag", tag_id);
    }
    serializer.append_pair("limit", &limit_value(limit));
    if operation == Operation::Or {
        serializer.append_pair("operation", "or");
    }
    format!("/media?{}", serializer.finish())
}

fn operation_button(
    label: &str,
    target: Operation,
    current: Operation,
    tag_ids: &[String],
    limit: usize,
) -> Markup {
    let active = target == current;
    html! {
        form class="operation-form" method="get" action="/media" {
            @for id in tag_ids {
                input type="hidden" name="tag" value=(id);
            }
            input type="hidden" name="limit" value=(limit_value(limit));
            @if target == Operation::Or {
                input type="hidden" name="operation" value="or";
            }
            button
                type="submit"
                class=(if active { "operation-button is-selected" } else { "operation-button" })
                aria-pressed=(active) {
                (label)
            }
        }
    }
}

fn operation_input(operation: Operation) -> Markup {
    html! {
        @if operation == Operation::Or {
            input type="hidden" name="operation" value="or";
        }
    }
}

fn query_results(items: &MediaQueryItems, operation: Operation) -> Markup {
    match items {
        MediaQueryItems::Count(count) => html! {
            div class="media-count-result" {
                span class="media-count-number" { (count) }
                p { "matching media items" }
                p class="media-control-note" { "Lower the display limit to show individual items." }
            }
        },
        MediaQueryItems::Items(items) if items.is_empty() => html! {
            p class="media-empty-state" {
                @if operation == Operation::Or {
                    "No media matches any selected tags."
                } @else {
                    "No media matches all selected tags."
                }
            }
        },
        MediaQueryItems::Items(items) => html! {
            p class="media-result-summary" {
                "Showing " (items.len()) " " (if items.len() == 1 { "item" } else { "items" })
            }
            div class="media-item-grid" {
                @for item in items {
                    (media_card(item))
                }
            }
        },
    }
}

fn relationship_picker(selected_ids: &[String], limit: usize) -> Markup {
    let selected = selected_ids.join(",");
    let limit = limit_value(limit);
    html! {
        form
            id="media-relationship-search"
            class="relationship-search media-relationship-search"
            data-signals="{ mediaquery: '' }"
            data-selected=(selected)
            data-limit=(limit)
            data-on:submit__prevent="@get('/media/relationship-submit?query='+encodeURIComponent($mediaquery)+'&selected='+encodeURIComponent(el.dataset.selected)+'&limit='+el.dataset.limit)" {
            label for="media-relationship-query" class="explore-search-label" {
                "SEARCH RELATIONSHIPS"
            }
            input
                id="media-relationship-query"
                type="search"
                name="query"
                placeholder="Try “place” or “to do”"
                autocomplete="off"
                aria-controls="media-relationship-suggestions"
                data-selected=(selected)
                data-limit=(limit)
                data-bind:mediaquery=""
                data-on:input="@get('/media/relationship-search?prefix='+encodeURIComponent($mediaquery)+'&selected='+encodeURIComponent(el.dataset.selected)+'&limit='+el.dataset.limit)";
        }

        div
            id="media-relationship-suggestions"
            class="relationship-suggestions media-picker-suggestions"
            role="status"
            aria-live="polite" {
            p class="explore-hint" { "Type to search relationships, then choose a tag to add it." }
        }

        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-hint" { "Choose a relationship to see available tags." }
        }
    }
}

pub fn picker_suggestions(
    prefix: &str,
    relationships: &[String],
    selected_ids: &[String],
    limit: usize,
) -> Markup {
    let selected = selected_ids.join(",");
    let limit = limit_value(limit);
    html! {
        div id="media-relationship-suggestions" class="relationship-suggestions media-picker-suggestions" role="status" aria-live="polite" {
            @if prefix.trim().is_empty() {
                p class="explore-hint" { "Type to search relationships, then choose a tag to add it." }
            } @else if relationships.is_empty() {
                p class="explore-hint" { "No relationships match “" (prefix) "”." }
            } @else {
                p class="suggestion-count" {
                    "RELATIONSHIPS"
                    span { (relationships.len()) }
                }
                ul class="relationship-option-list" {
                    @for relationship in relationships {
                        li {
                            button
                                type="button"
                                class="relationship-option"
                                data-relationship=(relationship)
                                data-selected=(selected)
                                data-limit=(limit)
                                data-on:click="@get('/media/relationship?name='+encodeURIComponent(el.dataset.relationship)+'&selected='+encodeURIComponent(el.dataset.selected)+'&limit='+el.dataset.limit)" {
                                span { (relationship) }
                                span class="relationship-option-arrow" aria-hidden="true" { "↗" }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn picker_relationship_tags(
    relationship: &str,
    tags: &[Tag],
    selected_ids: &[String],
    limit: usize,
) -> Markup {
    let available = tags
        .iter()
        .filter(|tag| !selected_ids.iter().any(|id| id == tag.id()))
        .collect::<Vec<_>>();
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            article class="relationship-result-card media-picker-result-card" {
                div class="relationship-result-heading" {
                    div {
                        p class="eyebrow" { "RELATIONSHIP" }
                        h2 { (relationship) }
                    }
                    span class="relationship-tag-count" { (available.len()) " available" }
                }
                @if available.is_empty() {
                    p class="explore-hint" { "No unselected tags are available in this relationship." }
                } @else {
                    div class="relationship-tag-grid" {
                        @for tag in available {
                            a
                                class="relationship-tag-card media-picker-tag"
                                data-testid="or-picker-tag"
                                href=(media_url(
                                    &selected_ids
                                        .iter()
                                        .cloned()
                                        .chain(std::iter::once(tag.id().to_string()))
                                        .collect::<Vec<_>>(),
                                    limit,
                                    Operation::Or,
                                )) {
                                span class="relationship-tag-mark" aria-hidden="true" { "+" }
                                strong { (tag.name()) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn limit_value(limit: usize) -> String {
    if limit == usize::MAX {
        "unlimited".to_string()
    } else {
        limit.to_string()
    }
}

pub fn picker_suggestion_error(message: &str) -> Markup {
    html! {
        div id="media-relationship-suggestions" class="relationship-suggestions media-picker-suggestions" role="status" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn picker_relationship_error(message: &str) -> Markup {
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn picker_message(message: &str) -> Markup {
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-hint" { (message) }
        }
    }
}

pub fn render_details(media: &Media, tags: &[Tag]) -> Markup {
    let asset_url = format!("/asset/{}", media.id.as_str());
    let title = media.title.as_deref().unwrap_or_else(|| media.id.as_str());
    let mime_type = media
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");
    let size_display = match media.file_size {
        Some(s) if s < 1024 => format!("{s} B"),
        Some(s) if s < 1024 * 1024 => format!("{:.1} KB", s as f64 / 1024.0),
        Some(s) => format!("{:.2} MB", s as f64 / (1024.0 * 1024.0)),
        None => "Unknown".to_string(),
    };
    let download_name = media
        .url
        .to_file_path()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| media.id.as_str().to_string());

    layout::render(
        "Media details",
        html! {
            section class="shell media-details section-space" {
                button class="back-button" data-on:click="window.history.back()" { "← Back" }
                div class="details-meta" {
                    h1 { (title) }
                    p { "MIME: " (mime_type) }
                    p { "Size: " (size_display) }
                    p { "Views: " (media.view_count) }
                    p { "URL: " (media.url.as_str()) }
                    div class="tag-list" {
                        @for tag in tags {
                            span class="tag-chip" { (tag.name()) }
                        }
                    }
                }
                div class="details-media" {
                    @if mime_type.starts_with("image/") {
                        img class="details-image" src=(asset_url) alt=(title) data-on:click="el.classList.toggle('full-size')" {}
                    } @else if mime_type.starts_with("video/") {
                        video controls preload="metadata" src=(asset_url) { "Your browser does not support embedded video." }
                    } @else {
                        a class="media-download" href=(asset_url) download=(download_name) {
                            span class="download-icon" aria-hidden="true" { "↓" }
                            span { "Download file" }
                        }
                    }
                }
            }
            script src="/media.js" {}
        },
    )
}

fn media_card(item: &Media) -> Markup {
    let asset_url = format!("/asset/{}", item.id.as_str());
    let title = item.title.as_deref().unwrap_or_else(|| item.id.as_str());
    let download_name = item
        .url
        .to_file_path()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| item.id.as_str().to_string());
    let mime_type = item
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    html! {
        article class="media-item-card" data-testid="media-card" {
            div class="media-preview" {
                @if mime_type.starts_with("image/") {
                    img
                        src=(asset_url)
                        alt=(title)
                        loading="lazy"
                        data-media-id=(item.id.as_str())
                        data-on:click="window.openMediaLightbox(el)";
                } @else if mime_type.starts_with("video/") {
                    video controls preload="metadata" src=(asset_url) {
                        "Your browser does not support embedded video."
                    }
                } @else {
                    a class="media-download" href=(asset_url) download=(download_name) {
                        span class="download-icon" aria-hidden="true" { "↓" }
                        span { "Download file" }
                    }
                }
            }
            div class="media-item-info" {
                h3 { (title) }
                p { (mime_type) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recondo_domain::model::{MediaId, MediaOpts};

    #[test]
    fn video_media_uses_native_player_controls() {
        let media = Media::new(
            MediaId::from_hash("video-id").unwrap(),
            url::Url::parse("file:///assets/video.mp4").unwrap(),
            Some(MediaOpts {
                title: Some("video.mp4".into()),
                mime_type: Some("video/mp4".into()),
                file_size: Some(100),
                view_count: None,
            }),
        );

        let html = media_card(&media).into_string();

        assert!(html.contains("<video controls"));
        assert!(html.contains("src=\"/asset/video-id\""));
        assert!(!html.contains("Download file"));
    }
}
