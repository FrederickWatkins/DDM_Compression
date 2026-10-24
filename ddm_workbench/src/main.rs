//! DDM GUI built with Dioxus
use components::{Hero, PlayButton};
use dioxus::prelude::*;

use crate::components::{WindowDecorations, WindowMenu, WindowMenuItem};

/// Define a components module that contains all shared components for our app.
mod components;

// The asset macro also minifies some assets like CSS and JS to make bundled smaller
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const THEMES_CSS: Asset = asset!("/assets/themes.css");

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
    let mut show_open_dialog = use_signal(|| false);

    rsx! {
        Stylesheet { href: TAILWIND_CSS }
        Stylesheet { href: THEMES_CSS }
        Stylesheet { href: MAIN_CSS }

        div {
            class: "flex min-h-screen flex-col bg-app-bg text-app-text",

            WindowDecorations {
                title: "DDM Workbench".to_string(),

                WindowMenu {
                    label: "File".to_string(),

                    WindowMenuItem {
                        label: "Open…".to_string(),
                        on_select: move |_| show_open_dialog.set(true),
                    }

                    WindowMenuItem {
                        label: "Close".to_string(),
                        on_select: move |_| {},
                    }
                }

                WindowMenu {
                    label: "View".to_string(),

                    WindowMenuItem {
                        label: "Reset layout".to_string(),
                        on_select: move |_| {},
                    }
                }
            }

            main {
                class: "min-w-0 flex-1 p-6",

                h1 {
                    class: "text-2xl font-bold text-app-primary",
                    "Dashboard"
                }
            }
        }
    }
}