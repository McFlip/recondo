use maud::{Markup, html};
use recondo_domain::model::RecycledMedia;

use crate::layout;

pub fn render_recycle_bin(
    items: &[RecycledMedia],
    notice: Option<&str>,
    error: Option<&str>,
) -> Markup {
    layout::render(
        "Recycle bin",
        html! {
            section class="shell media-shell recycle-bin-shell section-space" {
                div class="recycle-bin-header-row" {
                    div class="section-heading media-heading" {
                        p class="eyebrow" { "Temporary hold" }
                        h1 { "Recycle " em { "bin." } }
                        p class="section-description" {
                            "Restore items with their tags or delete them permanently from storage."
                        }
                    }
                    @if !items.is_empty() {
                        form class="recycle-empty-form" data-on:submit__prevent="@post('/recycle-bin/empty', {contentType: 'form'})" {
                            button type="submit" class="operation-button recycle-danger-button" {
                                "Empty recycle bin"
                            }
                        }
                    }
                }

                @if let Some(notice) = notice {
                    p class="details-tag-notice" role="status" { (notice) }
                }
                @if let Some(error) = error {
                    p class="details-tag-error" role="alert" { (error) }
                }

                @if items.is_empty() {
                    div class="media-empty-state" data-testid="recycle-bin-empty" {
                        p class="media-muted" { "The recycle bin is empty." }
                    }
                } @else {
                    div class="media-grid recycle-bin-grid" {
                        @for item in items {
                            (recycled_card(item))
                        }
                    }
                }
            }
        },
    )
}

fn recycled_card(item: &RecycledMedia) -> Markup {
    let media = &item.media;
    let id_str = media.id.as_str();
    let asset_url = format!("/asset/{id_str}");
    let title = media.title.as_deref().unwrap_or(id_str);
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
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| id_str.to_string());

    html! {
        article class="media-item-card recycle-item-card" data-testid="recycled-media-card" data-media-id=(id_str) {
            div class="media-preview" {
                @if mime_type.starts_with("image/") {
                    img src=(asset_url) alt=(title) loading="lazy";
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
            div class="media-item-info recycle-item-info" {
                h3 { (title) }
                p { (mime_type) " · " (size_display) }
                @if !item.tags.is_empty() {
                    div class="tag-list recycle-tag-list" {
                        @for (relationship, tag) in &item.tags {
                            span class="tag-chip" { (relationship) ": #" (tag.name()) }
                        }
                    }
                }
                div class="recycle-item-actions" {
                    form data-on:submit__prevent="@post('/recycle-bin/restore', {contentType: 'form'})" {
                        input type="hidden" name="media_id" value=(id_str);
                        button type="submit" class="operation-button" { "Restore" }
                    }
                    form data-on:submit__prevent="@post('/recycle-bin/delete', {contentType: 'form'})" {
                        input type="hidden" name="media_id" value=(id_str);
                        button type="submit" class="operation-button recycle-danger-button" { "Delete permanently" }
                    }
                }
            }
        }
    }
}
