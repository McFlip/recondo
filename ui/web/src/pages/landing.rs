use maud::{Markup, html};

use crate::{components, layout};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoFilter {
    All,
    Data,
    Spreadsheets,
    Code,
    Memes,
}

impl DemoFilter {
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "all" => Some(Self::All),
            "data" => Some(Self::Data),
            "spreadsheets" => Some(Self::Spreadsheets),
            "code" => Some(Self::Code),
            "memes" => Some(Self::Memes),
            _ => None,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Data => "data",
            Self::Spreadsheets => "spreadsheets",
            Self::Code => "code",
            Self::Memes => "memes",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All media",
            Self::Data => "Data",
            Self::Spreadsheets => "Spreadsheets",
            Self::Code => "Code",
            Self::Memes => "Memes",
        }
    }

    fn matches(self, item: &DemoMedia) -> bool {
        match self {
            Self::All => true,
            Self::Data => item.tags.contains(&"data"),
            Self::Spreadsheets => item.tags.contains(&"spreadsheet"),
            Self::Code => item.tags.contains(&"code"),
            Self::Memes => item.tags.contains(&"meme"),
        }
    }
}

struct DemoMedia {
    title: &'static str,
    filename: &'static str,
    tags: &'static [&'static str],
}

const DEMO_MEDIA: [DemoMedia; 4] = [
    DemoMedia {
        title: "Data Trap",
        filename: "data_trap.png",
        tags: &["data", "security"],
    },
    DemoMedia {
        title: "Spreadsheets",
        filename: "spreadsheets.png",
        tags: &["spreadsheet", "data"],
    },
    DemoMedia {
        title: "Excel + Lambda",
        filename: "excel_lambda_new.png",
        tags: &["spreadsheet", "code"],
    },
    DemoMedia {
        title: "Exploits of a Mom",
        filename: "exploits_of_a_mom.png",
        tags: &["meme", "security"],
    },
];

pub fn render() -> Markup {
    layout::render(
        "Discover more in your media collection",
        html! {
            section class="hero shell" {
                div class="hero-copy" {
                    p class="eyebrow" { span class="eyebrow-dot" {} "A new way to explore your collection" }
                    h1 { "Find the thread." br; em { "Follow it anywhere." } }
                    p class="hero-description" {
                        "Recondo connects your media through the details that matter to you. Search with tags, follow relationships, and discover what a folder could never show you."
                    }
                    div class="hero-actions" {
                        a class="button button-primary" href="#demo" { "Explore the demo" span aria-hidden="true" { "↘" } }
                        a class="text-link" href="#approach" { "Get to know Recondo" }
                    }
                    div class="hero-proof" {
                        span class="proof-icon" aria-hidden="true" { "↗" }
                        p { strong { "No rigid hierarchy." } " Just meaningful connections." }
                    }
                }
                div class="hero-art" aria-label="An illustration of connected tags" role="img" {
                    div class="orbit orbit-one" {}
                    div class="orbit orbit-two" {}
                    div class="orbit orbit-three" {}
                    div class="orbit-center" {
                        span class="orbit-symbol" aria-hidden="true" { "✳" }
                        span { "your collection" }
                    }
                    span class="orbit-tag orbit-tag-top" { "landscape" }
                    span class="orbit-tag orbit-tag-right" { "golden hour" }
                    span class="orbit-tag orbit-tag-bottom" { "coast" }
                    span class="orbit-tag orbit-tag-left" { "35mm" }
                    span class="orbit-tag orbit-tag-inner" { "travel" }
                    span class="orbit-node node-one" {}
                    span class="orbit-node node-two" {}
                    span class="orbit-node node-three" {}
                }
                div class="hero-caption" { "A collection is more than its folders." }
            }

            section class="intro-strip" {
                div class="shell intro-inner" {
                    p class="eyebrow" { "RECONDO /ˌrɛkɒnˈdoʊ/" }
                    p { "A graph-based media discovery tool for people who remember what they saw—not where they put it." }
                }
            }

            section id="approach" class="approach-section shell section-space" {
                div class="section-heading" {
                    p class="eyebrow" { "A different way to look" }
                    h2 { "Your collection, " em { "without the dead ends." } }
                    p class="section-description" {
                        "Folders ask you to pick one place. Recondo lets every item belong to all the ideas it connects to."
                    }
                }
                div class="principle-grid" {
                    (principle("01", "Describe, don’t file.", "Add tags for what something is, where it belongs, or what it reminds you of. One item can connect to many ideas."))
                    (principle("02", "Combine what you know.", "Use AND, OR, and NOT to narrow or broaden a search across independent dimensions of your collection."))
                    (principle("03", "Follow the connection.", "Drill down into a detail or pivot to a different tag—explore without losing your place."))
                }
            }

            section id="demo" class="demo-section" {
                div class="shell section-space" {
                    div class="demo-heading" {
                        div {
                            p class="eyebrow" { "A small field test" }
                            h2 { "A few tags can change " em { "everything." } }
                        }
                        p class="section-description" {
                            "Choose a tag to see how a connection can surface different media. These example items come from the project’s test fixtures."
                        }
                    }
                    div class="demo-panel" {
                        div class="demo-toolbar" {
                            div {
                                span class="demo-label" { "EXPLORE BY TAG" }
                                (filter_controls(DemoFilter::All))
                            }
                            p class="demo-hint" { "Select a tag to update the collection ↗" }
                        }
                        (results(DemoFilter::All))
                    }
                    p class="demo-disclaimer" { "Illustrative fixture data · Real collections, real connections, coming as Recondo develops." }
                }
            }

            section class="closing-section shell" {
                div class="closing-mark" aria-hidden="true" { "✳" }
                p class="eyebrow" { "UNDER DEVELOPMENT" }
                h2 { "A little more discovery. " em { "A lot less hunting." } }
                p { "Recondo is taking shape. The idea is simple: finding media should feel like exploration, not a scavenger hunt." }
                a class="button button-secondary" href="#demo" { "Try the demo" span aria-hidden="true" { "↗" } }
            }
        },
    )
}

pub fn results(filter: DemoFilter) -> Markup {
    let items = DEMO_MEDIA
        .iter()
        .filter(|item| filter.matches(item))
        .collect::<Vec<_>>();

    html! {
        section id="demo-results" class="results-section" aria-live="polite" {
            div class="results-summary" {
                p { "Showing " strong { (items.len()) } " " (if items.len() == 1 { "item" } else { "items" }) }
                span class="active-filter" { "#" (filter.label().to_lowercase().replace(' ', "-")) }
            }
            div class="media-grid" {
                @for item in items {
                    (components::media_card(item.title, item.filename, item.tags))
                }
            }
        }
    }
}

pub fn filter_controls(selected: DemoFilter) -> Markup {
    html! {
        div id="demo-filters" class="filter-list" aria-label="Filter demo media by tag" {
            @for filter in [DemoFilter::All, DemoFilter::Data, DemoFilter::Spreadsheets, DemoFilter::Code, DemoFilter::Memes] {
                (filter_button(filter, selected))
            }
        }
    }
}

fn filter_button(filter: DemoFilter, selected: DemoFilter) -> Markup {
    let class = if filter == selected {
        "filter-button is-selected"
    } else {
        "filter-button"
    };
    let action = format!("@get('/demo/{}')", filter.slug());

    html! {
        button type="button" class=(class) aria-pressed=(filter == selected) data-on:click=(action) {
            (filter.label())
        }
    }
}

fn principle(number: &str, title: &str, description: &str) -> Markup {
    html! {
        article class="principle-card" {
            span class="principle-number" { (number) }
            h3 { (title) }
            p { (description) }
        }
    }
}
