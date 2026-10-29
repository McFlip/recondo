use maud::{DOCTYPE, Markup, html};

use crate::components;

pub fn render(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="theme-color" content="#11130f";
                title { (title) " · Recondo" }
                link rel="stylesheet" href="/styles.css";
                script type="module" src="https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.4/bundles/datastar.js" {}
            }
            body {
                (components::site_header())
                main { (content) }
                (components::site_footer())
            }
        }
    }
}
