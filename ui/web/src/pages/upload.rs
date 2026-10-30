use maud::{Markup, html};
use recondo_domain::model::Tag;

use crate::layout;

pub fn render() -> Markup {
    layout::render(
        "Upload media",
        html! {
            section class="shell upload-shell section-space" {
                div class="section-heading upload-heading" {
                    p class="eyebrow" { "Media Ingestion" }
                    h1 { "Upload " em { "media files." } }
                    p class="section-description" {
                        "Choose relationship/tag pairs, then select or drag files. Every pair applies to each file in this upload."
                    }
                }

                div id="upload-panel" class="upload-panel" {
                    form id="upload-form"
                        class="upload-form"
                        enctype="multipart/form-data"
                        data-signals="{ dragging: false, uploading: false, errored: false, totalFiles: 0, processedCount: 0, relationshipquery: '' }"
                        data-on:datastar-fetch="
                            if (evt.detail.type === 'started') {
                                $errored = false;
                                $processedCount = 0;
                                $uploading = true;
                                const ph = document.getElementById('feed-placeholder');
                                if (ph) ph.remove();
                            } else if (evt.detail.type === 'finished') {
                                $uploading = false;
                                const fileInput = document.getElementById('file-input');
                                if (fileInput) fileInput.value = '';
                            } else if (evt.detail.type === 'error') {
                                $errored = true;
                            }
                        " {
                        section class="upload-tagging" aria-label="Tags for uploaded media" {
                            div class="upload-tagging-heading" {
                                div {
                                    p class="eyebrow" { "TAG YOUR UPLOAD" }
                                    h2 { "Choose connections" }
                                    p class="processed-hint" {
                                        "Selected pairs apply to every file. Leave this empty to keep the default “needs tags” pair."
                                    }
                                }
                                script type="module" src="/upload.js" {}
                            }

                            div class="selected-upload-tags-section" {
                                p class="media-control-label" { "SELECTED TAGS" }
                                div id="selected-upload-tags" class="selected-upload-tags" aria-live="polite" {
                                    p id="upload-tag-empty" class="media-muted" {
                                        "No additional tags selected."
                                    }
                                }
                                p id="upload-tag-error" class="explore-error" role="alert" {}
                            }

                            div class="upload-tag-picker" {
                                div class="upload-existing-tag-picker" {
                                    label for="upload-relationship-query" class="explore-search-label" {
                                        "FIND AN EXISTING RELATIONSHIP"
                                    }
                                    input
                                        id="upload-relationship-query"
                                        class="upload-relationship-query"
                                        type="search"
                                        placeholder="Try “place” or “to do”"
                                        autocomplete="off"
                                        aria-controls="upload-relationship-suggestions"
                                        data-bind:relationshipquery=""
                                        data-on:input="@get('/upload/relationship-search?prefix='+encodeURIComponent($relationshipquery))";
                                    div
                                        id="upload-relationship-suggestions"
                                        class="upload-relationship-suggestions"
                                        role="status"
                                        aria-live="polite" {
                                        p class="explore-hint" { "Search for a relationship to see its tags." }
                                    }
                                    div
                                        id="upload-relationship-details"
                                        class="upload-relationship-details"
                                        aria-live="polite" {
                                        p class="explore-hint" { "Choose a relationship to add one of its tags." }
                                    }
                                }

                                fieldset class="new-upload-tag-pair" {
                                    legend { "Create a relationship and tag" }
                                    p class="processed-hint" {
                                        "Both fields are required; a relationship and tag are always added together."
                                    }
                                    label for="new-upload-relationship" { "Relationship" }
                                    input
                                        id="new-upload-relationship"
                                        type="text"
                                        placeholder="e.g. location"
                                        autocomplete="off";
                                    label for="new-upload-tag" { "Tag" }
                                    input
                                        id="new-upload-tag"
                                        type="text"
                                        placeholder="e.g. kitchen"
                                        autocomplete="off";
                                    button
                                        type="button"
                                        class="operation-button upload-add-tag"
                                        data-on:click="window.addUploadTagPair(document.getElementById('new-upload-relationship').value, document.getElementById('new-upload-tag').value)" {
                                        "Add pair"
                                    }
                                }
                            }
                        }

                        div id="dropzone"
                            class="upload-dropzone"
                            data-class:is-dragging="$dragging"
                            data-on:dragover__prevent="$dragging = true"
                            data-on:dragleave="if (!evt.relatedTarget || !el.contains(evt.relatedTarget)) { $dragging = false; }"
                            data-on:drop__prevent="
                                $dragging = false;
                                const dt = evt.dataTransfer;
                                if (dt && dt.files && dt.files.length > 0) {
                                    const input = document.getElementById('file-input');
                                    if (input) {
                                        input.files = dt.files;
                                        $totalFiles = dt.files.length;
                                        @post('/upload', {contentType: 'form'});
                                    }
                                }
                            " {

                            input
                                id="file-input"
                                class="upload-file-input"
                                type="file"
                                name="files"
                                multiple
                                data-on:change="
                                    if (el.files && el.files.length > 0) {
                                        $totalFiles = el.files.length;
                                        @post('/upload', {contentType: 'form'});
                                    }
                                ";

                            div class="dropzone-content" {
                                div class="dropzone-icon" aria-hidden="true" { "↑" }
                                p class="dropzone-title" { "Drag and drop files here, or click to browse" }
                                p class="dropzone-hint" { "Select one or more images or documents to add to your collection" }
                            }
                        }

                        div
                            id="upload-status"
                            class="upload-status"
                            data-class:is-errored="$errored"
                            data-show="$uploading || $processedCount > 0 || $errored" {
                            div class="progress-info" {
                                span class="progress-status-text"
                                    data-text="($errored ? 'Upload failed' : (!$uploading && $processedCount > 0 ? 'Upload complete' : ($processedCount > 0 ? 'Processing ' + Math.min($processedCount, $totalFiles) + ' of ' + $totalFiles : ($uploading ? 'Uploading files...' : 'Ready'))))"
                                    { "Ready" }
                                span class="progress-percentage"
                                    data-text="($totalFiles > 0 ? Math.min(100, Math.round(100 * $processedCount / $totalFiles)) + '%' : '0%')"
                                    { "0%" }
                            }
                            div class="progress-bar-container" {
                                div class="progress-bar"
                                    data-class:indeterminate="$uploading && $processedCount === 0"
                                    data-style:width="(($totalFiles > 0 ? Math.min(100, Math.round(100 * $processedCount / $totalFiles)) : 0) + '%')"
                                    {}
                            }
                        }
                    }

                    div class="processed-section" {
                        div class="processed-heading" {
                            div {
                                h2 { "Processed Media" }
                                p class="processed-hint" { "Media items will appear here in real-time as they are processed." }
                            }
                            button
                                type="button"
                                class="clear-upload-list"
                                data-on:click="window.clearProcessedUploads()" {
                                "Clear uploaded list"
                            }
                        }
                        div id="processed-feed" class="processed-feed" {
                            p id="feed-placeholder" class="feed-placeholder" { "No files uploaded yet. Select files above to begin." }
                        }
                    }
                }
            }
        },
    )
}

pub fn relationship_suggestions(prefix: &str, relationships: &[String]) -> Markup {
    html! {
        div id="upload-relationship-suggestions" class="upload-relationship-suggestions" role="status" aria-live="polite" {
            @if prefix.trim().is_empty() {
                p class="explore-hint" { "Search for a relationship to see its tags." }
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
                                data-on:click="@get('/upload/relationship?name='+encodeURIComponent(el.dataset.relationship))" {
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

pub fn relationship_tags(relationship: &str, tags: &[Tag]) -> Markup {
    html! {
        div id="upload-relationship-details" class="upload-relationship-details" aria-live="polite" {
            @if tags.is_empty() {
                p class="explore-hint" { "This relationship has no tags yet. Create a relationship/tag pair below." }
            } @else {
                p class="media-control-label" { "TAGS IN " (relationship) }
                div class="upload-relationship-tag-list" {
                    @for tag in tags {
                        button
                            type="button"
                            class="relationship-tag-card upload-existing-tag"
                            data-relationship=(relationship)
                            data-tag=(tag.name())
                            data-on:click="window.addUploadTagPair(el.dataset.relationship, el.dataset.tag)" {
                            span class="relationship-tag-mark" aria-hidden="true" { "+" }
                            strong { (tag.name()) }
                        }
                    }
                }
            }
        }
    }
}

pub fn relationship_error(message: &str) -> Markup {
    html! {
        div id="upload-relationship-suggestions" class="upload-relationship-suggestions" role="status" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn relationship_tags_error(message: &str) -> Markup {
    html! {
        div id="upload-relationship-details" class="upload-relationship-details" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

/// Failure card for the processed-media feed: one per file (or stream) that
/// could not be ingested, so errors are visible in the UI instead of only in
/// the server log.
pub fn media_error_card(filename: &str, reason: &str) -> Markup {
    html! {
        article class="processed-card processed-card-failed" {
            div class="processed-card-header" {
                span class="status-tag status-failed" { "FAILED" }
            }
            div class="processed-card-body" {
                h3 class="media-filename" title=(filename) { (filename) }
                p class="media-error" { (reason) }
            }
        }
    }
}

pub fn media_processed_card(
    filename: &str,
    media_id: &str,
    status: &str,
    file_size: u64,
    mime_type: Option<&str>,
    is_duplicate: bool,
    tag_pairs: &[(String, String)],
) -> Markup {
    let size_display = if file_size < 1024 {
        format!("{file_size} B")
    } else if file_size < 1024 * 1024 {
        format!("{:.1} KB", file_size as f64 / 1024.0)
    } else {
        format!("{:.2} MB", file_size as f64 / (1024.0 * 1024.0))
    };

    let status_class = if is_duplicate {
        "status-tag status-duplicate"
    } else {
        "status-tag status-success"
    };

    let status_label = if is_duplicate {
        "DUPLICATE (EXISTING)"
    } else {
        status
    };

    let short_id = if media_id.len() > 16 {
        &media_id[..16]
    } else {
        media_id
    };

    html! {
        article class="processed-card" {
            div class="processed-card-header" {
                span class=(status_class) { (status_label) }
                span class="media-filesize" { (size_display) }
            }
            div class="processed-card-body" {
                h3 class="media-filename" title=(filename) { (filename) }
                div class="media-meta" {
                    span class="meta-item" {
                        span class="meta-label" { "ID: " }
                        code class="meta-hash" { (short_id) "..." }
                    }
                    @if let Some(mime) = mime_type {
                        span class="meta-item" {
                            span class="meta-label" { "TYPE: " }
                            span class="meta-value" { (mime) }
                        }
                    }
                }
                div class="processed-tags" {
                    @if tag_pairs.is_empty() {
                        span class="tag-chip" { "#needs tags" }
                        span class="tag-chip" { "#to do" }
                    } @else {
                        @for (relationship, tag) in tag_pairs {
                            span class="tag-chip" { "#" (tag) " · " (relationship) }
                        }
                    }
                }
            }
        }
    }
}
