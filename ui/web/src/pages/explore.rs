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
                        li {
                            button
                                type="button"
                                class="relationship-option"
                                data-relationship=(relationship)
                                data-on:click="@get('/explore/relationship?name='+encodeURIComponent(el.dataset.relationship))" {
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
                            article class="relationship-tag-card" {
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
