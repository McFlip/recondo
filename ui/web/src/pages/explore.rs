use maud::{Markup, html};
use recondo_domain::model::Tag;

use crate::layout;

pub fn render() -> Markup {
    layout::render(
        "Explore relationships",
        html! {
            section class="shell explore-shell section-space" {
                div class="section-heading explore-heading" {
                    p class="eyebrow" { "Follow a connection" }
                    h1 { "Explore your " em { "collection." } }
                    p class="section-description" {
                        "Search a relationship to see every tag connected to it."
                    }
                }

                div class="explore-panel" {
                    form
                        id="relationship-search"
                        class="relationship-search"
                        data-signals="{ query: '' }"
                        data-on:submit__prevent="@get('/explore/submit?query='+encodeURIComponent($query))" {
                        label for="relationship-query" class="explore-search-label" {
                            "SEARCH RELATIONSHIPS"
                        }
                        input
                            id="relationship-query"
                            type="search"
                            name="query"
                            placeholder="Try “place” or “to do”"
                            autocomplete="off"
                            aria-controls="relationship-suggestions"
                            data-bind:query=""
                            data-on:input="@get('/explore/search?prefix='+encodeURIComponent($query))";
                    }

                    div
                        id="relationship-suggestions"
                        class="relationship-suggestions"
                        role="status"
                        aria-live="polite" {
                        p class="explore-hint" { "Type to search your relationships." }
                    }

                    (selection_message("Choose a relationship to see its tags."))
                }
            }
        },
    )
}

pub fn suggestions(prefix: &str, relationships: &[String]) -> Markup {
    html! {
        div id="relationship-suggestions" class="relationship-suggestions" role="status" aria-live="polite" {
            @if prefix.trim().is_empty() {
                p class="explore-hint" { "Type to search your relationships." }
            } @else if relationships.is_empty() {
                p class="explore-hint" { "No relationships match “" (prefix) "”." }
            } @else {
                p class="suggestion-count" {
                    "RELATIONSHIPS"
                    span { (relationships.len()) }
                }
                ul class="relationship-option-list" {
                    @for relationship in relationships {
                        li class="relationship-option-row" {
                            button
                                type="button"
                                class="relationship-option"
                                data-relationship=(relationship)
                                data-on:click="@get('/explore/relationship?name='+encodeURIComponent(el.dataset.relationship))" {
                                span { (relationship) }
                                span class="relationship-option-arrow" aria-hidden="true" { "↗" }
                            }
                            a
                                class="relationship-manage-link"
                                href=(manage_relationship_href(relationship))
                                aria-label=(format!("Manage {relationship}"))
                                title="Manage relationship" {
                                span aria-hidden="true" { "⚙" }
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
        div id="relationship-details" class="relationship-details" aria-live="polite" {
            article class="relationship-result-card" {
                div class="relationship-result-heading" {
                    div {
                        p class="eyebrow" { "RELATIONSHIP" }
                        h2 { (relationship) }
                    }
                    span class="relationship-tag-count" {
                        (tags.len()) " " (if tags.len() == 1 { "tag" } else { "tags" })
                    }
                }
                @if tags.is_empty() {
                    p class="explore-hint" { "This relationship has no tags yet." }
                } @else {
                    div class="relationship-tag-grid" {
                        @for tag in tags {
                            a class="relationship-tag-card" href=(media_tag_href(tag.id())) {
                                span class="relationship-tag-mark" aria-hidden="true" { "#" }
                                strong { (tag.name()) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn media_tag_href(tag_id: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("tag", tag_id)
        .append_pair("limit", "10")
        .finish();
    format!("/media?{query}")
}

pub fn manage_relationship_href(relationship: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("name", relationship)
        .finish();
    format!("/relationship/manage?{query}")
}

pub fn render_manage(
    relationship: &str,
    tags: &[Tag],
    notice: Option<&str>,
    error: Option<&str>,
) -> Markup {
    let search_action = {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("source", relationship)
            .finish();
        format!(
            "@get('/relationship/manage/search?{query}&prefix='+encodeURIComponent($merge_query))"
        )
    };
    let submit_action = {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("source", relationship)
            .finish();
        format!(
            "@get('/relationship/manage/search?{query}&prefix='+encodeURIComponent($merge_query))"
        )
    };
    layout::render(
        &format!("Manage {relationship}"),
        html! {
            section class="shell explore-shell manage-relationship-shell section-space" {
                a class="back-button" href="/explore" { "← Back to Explore" }

                div class="section-heading explore-heading" {
                    p class="eyebrow" { "Manage relationship" }
                    h1 { (relationship) }
                    p class="section-description" {
                        "Review the tags connected to this relationship or merge it into another relationship."
                    }
                }

                @if let Some(notice) = notice {
                    p class="details-tag-notice" role="status" { (notice) }
                }
                @if let Some(error) = error {
                    p class="details-tag-error" role="alert" { (error) }
                }

                div class="manage-relationship-stack" {
                    (relationship_tags(relationship, tags))

                    section class="merge-relationship-section" aria-labelledby="merge-relationship-heading" {
                        div class="merge-relationship-heading" {
                            p class="eyebrow" { "CONSOLIDATE" }
                            h2 id="merge-relationship-heading" { "Merge into another " em { "relationship." } }
                            p class="section-description" {
                                "Search for a destination relationship. Merging copies all tags from “"
                                (relationship)
                                "” into the selected relationship and deletes “"
                                (relationship)
                                "”."
                            }
                        }

                        div class="merge-relationship-panel" {
                            form
                                id="merge-relationship-search"
                                class="relationship-search"
                                data-signals="{ merge_query: '' }"
                                data-on:submit__prevent=(submit_action) {
                                label for="merge-relationship-query" class="explore-search-label" {
                                    "SEARCH DESTINATION RELATIONSHIP"
                                }
                                input
                                    id="merge-relationship-query"
                                    type="search"
                                    name="merge_query"
                                    placeholder="Search a relationship to merge into"
                                    autocomplete="off"
                                    aria-controls="merge-relationship-suggestions"
                                    data-bind:merge_query=""
                                    data-on:input=(search_action);
                            }

                            div
                                id="merge-relationship-suggestions"
                                class="relationship-suggestions merge-relationship-suggestions"
                                role="status"
                                aria-live="polite" {
                                p class="explore-hint" { "Type to search for a destination relationship." }
                            }
                        }
                    }
                }
            }
        },
    )
}

pub fn merge_suggestions(source: &str, prefix: &str, relationships: &[String]) -> Markup {
    html! {
        div
            id="merge-relationship-suggestions"
            class="relationship-suggestions merge-relationship-suggestions"
            role="status"
            aria-live="polite" {
            @if prefix.trim().is_empty() {
                p class="explore-hint" { "Type to search for a destination relationship." }
            } @else if relationships.is_empty() {
                p class="explore-hint" { "No other relationships match “" (prefix) "”." }
            } @else {
                p class="suggestion-count" {
                    "DESTINATION RELATIONSHIPS"
                    span { (relationships.len()) }
                }
                ul class="relationship-option-list" {
                    @for destination in relationships {
                        li class="merge-option-row" {
                            span class="merge-option-name" { (destination) }
                            form method="post" action="/relationship/manage/merge" class="merge-option-form" {
                                input type="hidden" name="source" value=(source);
                                input type="hidden" name="destination" value=(destination);
                                button
                                    type="submit"
                                    class="button button-secondary relationship-merge-button"
                                    aria-label=(format!("Merge into {destination}")) {
                                    "Merge"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn merge_suggestion_error(message: &str) -> Markup {
    html! {
        div
            id="merge-relationship-suggestions"
            class="relationship-suggestions merge-relationship-suggestions"
            role="status"
            aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn selection_message(message: &str) -> Markup {
    html! {
        div id="relationship-details" class="relationship-details" aria-live="polite" {
            p class="explore-hint explore-empty" { (message) }
        }
    }
}

pub fn suggestion_error(message: &str) -> Markup {
    html! {
        div id="relationship-suggestions" class="relationship-suggestions" role="status" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn relationship_error(message: &str) -> Markup {
    html! {
        div id="relationship-details" class="relationship-details" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}
