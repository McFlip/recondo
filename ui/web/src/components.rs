use maud::{Markup, html};

pub fn site_header() -> Markup {
    html! {
        header class="site-header" {
            a class="wordmark" href="/" aria-label="Recondo home" {
                span class="wordmark-mark" aria-hidden="true" { "R" }
                "recondo"
            }
            nav aria-label="Main navigation" {
                a href="#approach" { "Approach" }
                a href="#demo" { "Explore" }
                a class="nav-cta" href="#demo" { "See how it works" }
            }
        }
    }
}

pub fn site_footer() -> Markup {
    html! {
        footer class="site-footer" {
            a class="wordmark footer-wordmark" href="/" {
                span class="wordmark-mark" aria-hidden="true" { "R" }
                "recondo"
            }
            p { "A more curious way to find what you’re looking for." }
            span class="footer-note" { "Under development · Apache-2.0" }
        }
    }
}

pub fn tag_chip(tag: &str) -> Markup {
    html! { span class="tag-chip" { "#" (tag) } }
}

pub fn media_card(title: &str, filename: &str, tags: &[&str]) -> Markup {
    html! {
        article class="media-card" {
            div class="media-card-image" {
                img src=(format!("/media/{filename}")) alt=(title) loading="lazy";
                span class="image-index" { "FIELD NOTE / " (filename.replace(".png", "").to_uppercase()) }
            }
            div class="media-card-content" {
                h4 { (title) }
                div class="tag-list" {
                    @for tag in tags {
                        (tag_chip(tag))
                    }
                }
            }
        }
    }
}
