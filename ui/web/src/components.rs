use maud::{Markup, html};

pub fn site_header() -> Markup {
    html! {
        header class="site-header" {
            a class="wordmark" href="/" aria-label="Recondo home" {
                img src="/mark.svg" width="300" height="70";
            }
            nav aria-label="Main navigation" {
                a class="nav-cta" href="/explore" { "Explore" }
                a class="nav-cta" href="/upload" { "Upload media" }
                a class="nav-cta" href="/collections" { "Collections" }
                a class="nav-cta" href="/recycle-bin" { "Recycle bin" }
            }
        }
    }
}

pub fn site_footer() -> Markup {
    html! {
        footer class="site-footer" {
            a class="wordmark footer-wordmark" href="/" {
                img src="/mark.svg" width="300" height="70";
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
