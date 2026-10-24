use dioxus::prelude::*;

const PLAY_ICON: Asset = asset!("/assets/icons/play.svg");
const PAUSE_ICON: Asset = asset!("/assets/icons/pause.svg");

#[component]
pub fn PlayButton(playing: bool, size: u32, on_toggle: EventHandler<()>) -> Element {
    let icon = if playing { PAUSE_ICON } else { PLAY_ICON };
    let label = if playing {
        "Pause playback"
    } else {
        "Start playback"
    };
    rsx! {
            button {
                class: "play-pause-button",
                style: "width: {size}px; height: {size}px;",
                r#type: "button",
                title: label,
                aria_label: label,
                onclick: move |_| on_toggle.call(()),

                span {
                    class: "play-pause-icon",
                    style: "
                    width: {size}px;
                    height: {size}px;
                    --icon-mask: url('{icon}');
                "
                }
            }
        }
}
