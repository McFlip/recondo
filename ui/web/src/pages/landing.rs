use maud::{Markup, html};

use crate::{components, layout};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DemoTag {
    All,
    Data,
    Spreadsheets,
    Code,
    Memes,
    Security,
}

impl DemoTag {
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "all" => Some(Self::All),
            "data" => Some(Self::Data),
            "spreadsheets" => Some(Self::Spreadsheets),
            "code" => Some(Self::Code),
            "memes" => Some(Self::Memes),
            "security" => Some(Self::Security),
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
            Self::Security => "security",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All media",
            Self::Data => "Data",
            Self::Spreadsheets => "Spreadsheets",
            Self::Code => "Code",
            Self::Memes => "Memes",
            Self::Security => "Security",
        }
    }

    fn tag_name(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Data => Some("data"),
            Self::Spreadsheets => Some("spreadsheet"),
            Self::Code => Some("code"),
            Self::Memes => Some("meme"),
            Self::Security => Some("security"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoSelection(pub Vec<DemoTag>);

impl DemoSelection {
    pub fn from_slug(slug: &str) -> Option<Self> {
        if slug == "all" || slug.is_empty() {
            return Some(Self(Vec::new()));
        }
        let mut tags = Vec::new();
        for part in slug.split(',') {
            let tag = DemoTag::from_slug(part.trim())?;
            if tag == DemoTag::All {
                return Some(Self(Vec::new()));
            }
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        Some(Self(tags))
    }

    pub fn slug(&self) -> String {
        if self.0.is_empty() {
            "all".to_string()
        } else {
            self.0
                .iter()
                .map(|tag| tag.slug())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    pub fn matches(&self, item: &DemoMedia) -> bool {
        self.0.iter().all(|tag| {
            tag.tag_name()
                .map(|name| item.tags.contains(&name))
                .unwrap_or(true)
        })
    }
}

pub(crate) struct DemoMedia {
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
                            h2 { "Combining multiple tags can change " em { "everything." } }
                        }
                        p class="section-description" {
                            "Choose tags to see how logical AND narrows the results. Selecting multiple tags keeps only the media that matches every selected tag, letting you drill down from a broad idea to a precise subset."
                        }
                    }
                    div class="demo-panel" {
                        div class="demo-toolbar" {
                            div {
                                span class="demo-label" { "EXPLORE BY TAG" }
                                (filter_controls(&DemoSelection(Vec::new())))
                            }
                            p class="demo-hint" { "Toggle tags on or off; selecting more tags narrows the results ↗" }
                        }
                        (results(&DemoSelection(Vec::new())))
                    }
                    p class="demo-disclaimer" { "Illustrative fixture data · Real collections, real connections, coming as Recondo develops." }
                }
            }

            section class="name-section shell section-space" {
                div class="section-heading" {
                    p class="eyebrow" { "THE NAME" }
                    h2 { "Reconnaissance " em { " commando." } }
                }
                p class="section-description" {
                    "Recondo is a portmanteau of reconnaissance and commando. The name became associated with the U.S. Army's long-range reconnaissance patrols during the Vietnam War, whose small teams ventured deep into enemy territory to observe, gather intelligence, and locate an elusive enemy."
                }
                p class="section-description" {
                    "Recondo borrows that spirit of exploration: venture into a vast, unfamiliar collection, follow the leads, uncover connections, and find what you're looking for—even when you don't know exactly where to start."
                }
                div class="hero-actions" {
                    a class="button button-secondary" href="#demo" { "Try the demo" span aria-hidden="true" { "↗" } }
                }
            }
        },
    )
}

pub fn results(selection: &DemoSelection) -> Markup {
    let items = DEMO_MEDIA
        .iter()
        .filter(|item| selection.matches(item))
        .collect::<Vec<_>>();

    html! {
        section id="demo-results" class="results-section" aria-live="polite" {
            div class="results-summary" {
                p { "Showing " strong { (items.len()) } " " (if items.len() == 1 { "item" } else { "items" }) }
                @if !selection.0.is_empty() {
                    span class="active-filter" { (selection.0.iter().map(|t| format!("#{}", t.label().to_lowercase())).collect::<Vec<_>>().join(" + ")) }
                }
            }
            div class="media-grid" {
                @for item in items {
                    (components::media_card(item.title, item.filename, item.tags))
                }
            }
        }
    }
}

pub fn filter_controls(selection: &DemoSelection) -> Markup {
    html! {
        div id="demo-filters" class="filter-list" aria-label="Filter demo media by tag" {
            @for tag in [DemoTag::Data, DemoTag::Spreadsheets, DemoTag::Code, DemoTag::Memes, DemoTag::Security] {
                (filter_button(tag, selection))
            }
        }
    }
}

fn filter_button(tag: DemoTag, selection: &DemoSelection) -> Markup {
    let selected = selection.0.contains(&tag);
    let mut next = selection.0.clone();
    if selected {
        next.retain(|t| *t != tag);
    } else {
        next.push(tag);
    }
    let next_slug = DemoSelection(next).slug();
    let action = format!("@get('/demo/{}')", next_slug);

    let class = if selected {
        "filter-button is-selected"
    } else {
        "filter-button"
    };

    html! {
        button type="button" class=(class) aria-pressed=(selected) data-on:click=(action) {
            (tag.label())
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

