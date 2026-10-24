use dioxus::prelude::*;

use crate::components::menubar::{
    Menubar, MenubarContent, MenubarItem, MenubarMenu, MenubarTrigger,
};

const LOGO: Asset = asset!("/assets/logo.svg");
const MINIMIZE: Asset = asset!("/assets/icons/minimize.svg");
const RESTORE_DOWN: Asset = asset!("/assets/icons/restore-down.svg");
const MAXIMIZE: Asset = asset!("/assets/icons/maximize.svg");
const CLOSE: Asset = asset!("/assets/icons/close.svg");

#[component]
pub fn WindowDecorations() -> Element {
    let window = dioxus::desktop::use_window();
    let mut is_maximized = use_signal(|| window.is_maximized());

    let on_minimize = {
        let window = window.clone();
        move |_| window.set_minimized(true)
    };

    let on_toggle_maximize = {
        let window = window.clone();

        move |_| {
            let maximized = !is_maximized();
            window.set_maximized(maximized);
            is_maximized.set(maximized);
        }
    };

    let on_close = {
        let window = window.clone();
        move |_| window.close()
    };

    let on_drag = move |_| window.drag();

    rsx! {
        div { class: "top-bar",

            div { class: "top-bar-left-content",

                img { src: LOGO, width: 26, height: 26 }

                Menubar {
                    MenubarMenu { index: 0usize,
                        MenubarTrigger { "File" }
                        MenubarContent {
                            MenubarItem {
                                index: 0usize,
                                value: "open".to_string(),
                                on_select: move |value| {
                                    info!("Selected value: {}", value);
                                },
                                "Open"
                            }
                            MenubarItem {
                                index: 1usize,
                                value: "export".to_string(),
                                on_select: move |value| {
                                    info!("Selected value: {}", value);
                                },
                                "Save"
                            }
                        }
                    }

                    MenubarMenu { index: 1usize,
                        MenubarTrigger { "Run" }
                        MenubarContent {
                            MenubarItem {
                                index: 0usize,
                                value: "benchmark".to_string(),
                                on_select: move |value| {
                                    info!("Selected value: {}", value);
                                },
                                "Benchmark"
                            }
                        }
                    }
                }
            }

            div { class: "top-bar-title", "DDM Workbench" }

            // Only this empty region initiates a window drag.
            div { class: "top-bar-drag-region", onmousedown: on_drag }

            TopBarButtons {
                on_minimize,
                on_toggle_maximize,
                is_maximized,
                on_close,
            }
        }
    }
}

#[component]
fn TopBarButtons(
    on_minimize: EventHandler<MouseEvent>,
    on_toggle_maximize: EventHandler<MouseEvent>,
    is_maximized: ReadSignal<bool>,
    on_close: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "top-bar-buttons",

            button {
                class: "top-bar-button",
                title: "Minimise",
                aria_label: "Minimise window",
                onclick: move |event| on_minimize.call(event),

                img { src: MINIMIZE, width: 20, height: 20 }
            }

            button {
                class: "top-bar-button",
                title: if is_maximized() { "Restore" } else { "Maximise" },
                aria_label: if is_maximized() { "Restore window" } else { "Maximise window" },
                onclick: move |event| on_toggle_maximize.call(event),

                img {
                    src: if is_maximized() { RESTORE_DOWN } else { MAXIMIZE },
                    width: 16,
                    height: 16,
                }
            }

            button {
                class: "top-bar-button close-button",
                title: "Close",
                aria_label: "Close window",
                onclick: move |event| on_close.call(event),

                img { src: CLOSE, width: 20, height: 20 }
            }
        }
    }
}
