use maud::{Markup, html};
use recondo_domain::model::{Media, Tag};
use recondo_domain::{CountedTag, MediaQueryItems, MediaQueryResult, RelationshipTagGroup};

use crate::layout;

pub const DEFAULT_LIMIT: usize = 10;
pub const LIMIT_OPTIONS: [usize; 8] = [10, 25, 50, 75, 100, 150, 200, 400];

pub fn render(
    selected_tags: &[Tag],
    limit: usize,
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
                    aside class="media-sidebar" aria-label="Related tags" {
                        div class="media-sidebar-heading" {
                            p class="eyebrow" { "DISCOVER" }
                            h2 { "Related tags" }
                        }
                        nav class="relationship-accordion-list" aria-label="Tags grouped by relationship" {
                            @if let Some(result) = result {
                                @for group in &result.relationships {
                                    (relationship_group(group, &tag_ids, limit))
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

                    div class="media-main" {
                        nav class="media-toolbar" aria-label="Media search controls" {
                            section class="selected-tags-section" aria-label="Selected tags" {
                                p class="media-control-label" { "SELECTED TAGS" }
                                @if selected_tags.is_empty() {
                                    p class="media-muted" { "No tags selected" }
                                } @else {
                                    div class="selected-tag-list" {
                                        @for tag in selected_tags {
                                            (selected_tag(tag, &tag_ids, limit))
                                        }
                                    }
                                }
                            }

                            section class="media-controls-section" aria-label="Search options" {
                                div class="operation-control" {
                                    p class="media-control-label" { "LOGICAL OPERATION" }
                                    div class="operation-buttons" {
                                        button type="button" class="operation-button is-selected" aria-pressed="true" { "AND" }
                                        button type="button" class="operation-button" disabled { "OR" }
                                        button type="button" class="operation-button" disabled { "AND then OR" }
                                        button type="button" class="operation-button" disabled { "OR then AND" }
                                    }
                                    p class="media-control-note" { "Other logical operations are not functional yet." }
                                }
                                form class="limit-control" method="get" action="/media" {
                                    @for id in &tag_ids {
                                        input type="hidden" name="tag" value=(id);
                                    }
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
                                (query_results(&result.media))
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
                img id="media-lightbox-image" alt="Full-size media preview";
            }
        },
    )
}

fn relationship_group(
    group: &RelationshipTagGroup,
    selected_ids: &[String],
    limit: usize,
) -> Markup {
    html! {
        details class="relationship-accordion" {
            summary {
                span { (group.relationship) }
                span class="relationship-accordion-count" { (group.tags.len()) }
            }
            div class="relationship-accordion-tags" {
                @for counted_tag in &group.tags {
                    (related_tag(counted_tag, selected_ids, limit))
                }
            }
        }
    }
}

fn related_tag(counted_tag: &CountedTag, selected_ids: &[String], limit: usize) -> Markup {
    let href = media_url(
        &selected_ids
            .iter()
            .cloned()
            .chain(std::iter::once(counted_tag.tag.id().to_string()))
            .collect::<Vec<_>>(),
        limit,
    );
    html! {
        a class="related-tag-link" href=(href) {
            span { (counted_tag.tag.name()) }
            span class="related-tag-count" { (counted_tag.count) }
        }
    }
}

fn selected_tag(tag: &Tag, selected_ids: &[String], limit: usize) -> Markup {
    let remaining = selected_ids
        .iter()
        .filter(|id| id.as_str() != tag.id())
        .cloned()
        .collect::<Vec<_>>();
    let href = if remaining.is_empty() {
        "/explore".to_string()
    } else {
        media_url(&remaining, limit)
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

fn media_url(tag_ids: &[String], limit: usize) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for tag_id in tag_ids {
        serializer.append_pair("tag", tag_id);
    }
    if limit == usize::MAX {
        serializer.append_pair("limit", "unlimited");
    } else {
        serializer.append_pair("limit", &limit.to_string());
    }
    format!("/media?{}", serializer.finish())
}

fn query_results(items: &MediaQueryItems) -> Markup {
    match items {
        MediaQueryItems::Count(count) => html! {
            div class="media-count-result" {
                span class="media-count-number" { (count) }
                p { "matching media items" }
                p class="media-control-note" { "Lower the display limit to show individual items." }
            }
        },
        MediaQueryItems::Items(items) if items.is_empty() => html! {
            p class="media-empty-state" { "No media matches all selected tags." }
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
                        data-on:click="
                            const dialog = document.getElementById('media-lightbox');
                            document.getElementById('media-lightbox-image').src = el.src;
                            dialog.showModal();
                        ";
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
