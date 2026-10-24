
use dioxus::prelude::*;

#[component]
pub fn WindowDecorations(title: String, children: Element) -> Element {
    let window = dioxus::desktop::use_window();
    let maximized = use_signal(|| window.is_maximized());

    let drag_window = window.clone();
    let double_click_window = window.clone();
    let minimize_window = window.clone();
    let maximize_window = window.clone();
    let close_window = window.clone();

    let mut maximized_on_double_click = maximized;
    let mut maximized_on_button = maximized;

    rsx! {
        header {
            class: "relative z-50 flex h-10 w-full select-none items-stretch \
                    border-b border-app-text/10 bg-app-surface text-app-text",

            // Application menu bar
            nav {
                class: "flex shrink-0 items-center gap-1 px-2",
                {children}
            }

            // Draggable title area
            div {
                class: "flex min-w-0 flex-1 cursor-default items-center justify-center",
                onmousedown: move |_| drag_window.drag(),
                ondblclick: move |_| {
                    double_click_window.toggle_maximized();
                    maximized_on_double_click
                        .set(!maximized_on_double_click());
                },

                span {
                    class: "pointer-events-none truncate px-4 text-sm \
                            font-medium text-app-text/70",
                    "{title}"
                }
            }

            // Native-style window controls
            div {
                class: "flex shrink-0 items-stretch border-l border-app-text/10",

                // Minimize
                button {
                    r#type: "button",
                    class: "flex w-11 items-center justify-center \
                            transition-colors hover:bg-app-text/10 \
                            focus-visible:outline-none \
                            focus-visible:ring-2 \
                            focus-visible:ring-inset \
                            focus-visible:ring-app-primary",
                    title: "Minimize",
                    "aria-label": "Minimize window",
                    onclick: move |_| minimize_window.set_minimized(true),

                    svg {
                        class: "h-4 w-4",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        stroke_linecap: "round",
                        path { d: "M5 12h14" }
                    }
                }

                // Maximize / restore
                button {
                    r#type: "button",
                    class: "flex w-11 items-center justify-center \
                            transition-colors hover:bg-app-text/10 \
                            focus-visible:outline-none \
                            focus-visible:ring-2 \
                            focus-visible:ring-inset \
                            focus-visible:ring-app-primary",
                    title: if maximized() { "Restore" } else { "Maximize" },
                    "aria-label": if maximized() {
                        "Restore window"
                    } else {
                        "Maximize window"
                    },
                    onclick: move |_| {
                        maximize_window.toggle_maximized();
                        maximized_on_button.set(!maximized_on_button());
                    },

                    if maximized() {
                        svg {
                            class: "h-4 w-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            stroke_linejoin: "round",
                            path {
                                d: "M8 8V5a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-3"
                            }
                            rect {
                                x: "4",
                                y: "8",
                                width: "12",
                                height: "12",
                                rx: "1"
                            }
                        }
                    } else {
                        svg {
                            class: "h-4 w-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            rect {
                                x: "5",
                                y: "5",
                                width: "14",
                                height: "14",
                                rx: "1"
                            }
                        }
                    }
                }

                // Close
                button {
                    r#type: "button",
                    class: "flex w-11 items-center justify-center \
                            transition-colors hover:bg-app-text/10 \
                            focus-visible:outline-none \
                            focus-visible:ring-2 \
                            focus-visible:ring-inset \
                            focus-visible:ring-app-primary",
                    title: "Close",
                    "aria-label": "Close window",
                    onclick: move |_| close_window.close(),

                    svg {
                        class: "h-4 w-4",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        stroke_linecap: "round",
                        path { d: "M6 6l12 12M18 6L6 18" }
                    }
                }
            }
        }
    }
}

/// A dropdown menu in the custom title bar.
#[component]
pub fn WindowMenu(label: String, children: Element) -> Element {
    let mut open = use_signal(|| false);

    rsx! {
        div {
            class: "relative flex h-full items-center",

            button {
                r#type: "button",
                class: "h-full rounded-sm px-3 text-sm \
                        transition-colors hover:bg-app-text/10 \
                        focus-visible:outline-none \
                        focus-visible:ring-2 \
                        focus-visible:ring-app-primary",
                onclick: move |_| open.set(!open()),
                "aria-expanded": "{open}",
                "{label}"
            }

            if open() {
                div {
                    class: "absolute left-0 top-full z-50 min-w-44 \
                            rounded-md border border-app-text/10 \
                            bg-app-surface py-1 text-sm shadow-xl",
                    onclick: move |_| open.set(false),
                    {children}
                }
            }
        }
    }
}

/// A clickable item inside a [`WindowMenu`].
#[component]
pub fn WindowMenuItem(
    label: String,
    on_select: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "flex w-full items-center px-3 py-1.5 text-left \
                    hover:bg-app-text/10 \
                    focus-visible:outline-none \
                    focus-visible:bg-app-text/10",
            onclick: move |_| on_select.call(()),
            "{label}"
        }
    }
}
