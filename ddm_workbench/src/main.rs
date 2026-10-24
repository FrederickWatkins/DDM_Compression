//! DDM GUI built with Dioxus
use std::{io::BufReader, sync::Arc};

use ddm_app::load_l0b;
use dioxus::prelude::*;

/// Define a components module that contains all shared components for our app.
mod components;
use components::window_decorations::WindowDecorations;

use crate::components::ddm_player::DDMPlayer;

// The asset macro also minifies some assets like CSS and JS to make bundled smaller
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const DDM_WIDTH: usize = 20;
const DDM_HEIGHT: usize = 52;

type L0bFileData = ddm_app::L0bFileData<DDM_WIDTH, DDM_HEIGHT>;

fn main() {
    LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title("My Themed App")
                    .with_inner_size(dioxus::desktop::LogicalSize::new(900.0, 600.0))
                    .with_decorations(false),
            ),
        )
        .launch(App);
}

/// Main app view
#[component]
fn App() -> Element {
    let current_theme = use_signal(|| "dark".to_string());
    let reader = BufReader::new(std::fs::File::open("../data/HG1_TEST_512.l0b.ddm").unwrap());
    let file = Arc::new(load_l0b(reader, &mut prodash::progress::Discard).unwrap());

    use_effect(move || {
        let theme = current_theme();
        let script = format!(
            "document.documentElement.setAttribute('data-theme', '{}');",
            theme
        );

        spawn(async move {
            let _ = document::eval(&script).await;
        });
    });

    rsx! {
        document::Link {
            rel: "stylesheet",
            href: asset!("/assets/dx-components-theme.css"),
        }
        Stylesheet { href: TAILWIND_CSS }
        Stylesheet { href: MAIN_CSS }

        div {
            style: "width: 100%; height: 100%;",

            WindowDecorations {}

            div {
                DDMPlayer { file }
            }
        }
    }
}
