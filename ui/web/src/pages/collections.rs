use maud::{Markup, html};
use recondo_domain::model::Collection;

use crate::layout;

pub fn render_list(
    collections: &[(Collection, Vec<String>)],
    prefix: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> Markup {
    let prefix_json = format!("{:?}", prefix);
    layout::render(
        "Collections",
        html! {
            section class="shell collections-shell section-space" {
                div class="section-heading collections-heading" {
                    p class="eyebrow" { "Saved searches" }
                    h1 { "Your " em { "collections." } }
                    p class="section-description" {
                        "Reopen a saved media search with one click."
                    }
                }

                @if let Some(notice) = notice {
                    p class="details-tag-notice" role="status" { (notice) }
                }
                @if let Some(error) = error {
                    p class="details-tag-error" role="alert" { (error) }
                }

                form
                    class="relationship-search collections-search"
                    data-signals=(format!("{{ prefix: {} }}", prefix_json))
                    data-on:submit__prevent="@get('/collections/search?prefix='+encodeURIComponent($prefix))" {
                    label for="collection-prefix" class="explore-search-label" {
                        "SEARCH COLLECTIONS"
                    }
                    input
                        id="collection-prefix"
                        type="search"
                        name="prefix"
                        value=(prefix)
                        placeholder="Type a collection name"
                        autocomplete="off"
                        data-bind:prefix=""
                        data-on:input="@get('/collections/search?prefix='+encodeURIComponent($prefix))";
                    button
                        type="button"
                        class="button button-secondary"
                        data-on:click="$prefix=''; @get('/collections/search?prefix=')" {
                        "List all"
                    }
                }

                (results(collections, prefix))
            }
        },
    )
}

pub fn results(collections: &[(Collection, Vec<String>)], prefix: &str) -> Markup {
    html! {
        div id="collections-results" class="collections-results" {
            @if collections.is_empty() {
                p class="media-muted collections-empty" {
                    @if prefix.trim().is_empty() {
                        "No collections saved yet."
                    } @else {
                        "No collections match “" (prefix) "”."
                    }
                }
            } @else {
                div class="collections-grid" {
                    @for (collection, tag_names) in collections {
                        (collection_card(collection, tag_names))
                    }
                }
            }
        }
    }
}

fn collection_card(collection: &Collection, tag_names: &[String]) -> Markup {
    let view_url = collection_view_url(collection);
    let operation_label = match collection.operation.as_str() {
        "or" => "OR",
        "and_then_or" => "AND then OR",
        "or_then_and" => "OR then AND",
        _ => "AND",
    };
    html! {
        article class="collection-card" {
            div class="collection-card-heading" {
                h2 { (collection.name) }
                a class="button button-primary collection-view-button" href=(view_url) {
                    i class="fa-solid fa-folder-open" aria-hidden="true" {}
                    span { "View" }
                }
            }
            div class="collection-chips" {
                span class="operation-chip" { (operation_label) }
                @for name in tag_names {
                    span class="tag-chip" { "#" (name) }
                }
            }
            div class="collection-actions" {
                form method="post" action="/collections/delete" class="collection-delete-form" {
                    input type="hidden" name="name" value=(collection.name);
                    button
                        type="submit"
                        class="media-group-delete collection-delete-button"
                        aria-label="Delete collection"
                        title="Delete collection" {
                        svg class="media-trash-icon" viewBox="0 0 24 24" aria-hidden="true" {
                            path d="M4 7h16M10 11v6m4-6v6M5 7l1 14h12l1-14M9 7V4h6v3";
                        }
                    }
                }
            }
        }
    }
}

pub fn collection_view_url(collection: &Collection) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    if collection.groups.is_empty() {
        for tag_id in &collection.tag_ids {
            serializer.append_pair("tag", tag_id);
        }
        if collection.operation == "or" {
            serializer.append_pair("operation", "or");
        }
    } else {
        serializer.append_pair("operation", &collection.operation);
        for group in &collection.groups {
            serializer.append_pair("group", &group.join(","));
        }
    }
    let limit_value = if collection.limit == usize::MAX {
        "unlimited".to_string()
    } else {
        collection.limit.to_string()
    };
    serializer.append_pair("limit", &limit_value);
    format!("/media?{}", serializer.finish())
}