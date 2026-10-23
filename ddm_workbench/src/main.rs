//! DDM workbench
use ddm_workbench::app::App;
use iced::{Program, Size, window};
use prodash::{Root as _, tree::Item};
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

// -----------------------------------------------------------------------------
// Embedded SVG assets
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Theme
// -----------------------------------------------------------------------------

// const BACKGROUND: Color = Color::from_rgb8(22, 23, 32);
// const TEXT_COLOR: Color = Color::from_rgb8(233, 233, 242);
// const ACCENT: Color = Color::from_rgb8(185, 146, 255);

// fn app_theme(_: &App) -> Theme {
//     Theme::custom(
//         "DDM Workbench",
//         iced::theme::Palette {
//             background: BACKGROUND,
//             text: TEXT_COLOR,
//             primary: ACCENT,
//             success: Color::from_rgb8(119, 222, 186),
//             warning: Color::from_rgb8(255, 197, 120),
//             danger: Color::from_rgb8(255, 111, 139),
//         },
//     )
// }

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

fn main() -> iced::Result {
    let filter = EnvFilter::new("info,ddm_app=debug,ddm_workbench=debug");
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .pretty()
        .compact()
        .with_target(true)
        .with_file(false)
        .init();
    let root = prodash::tree::Root::new();
    
    iced::application(
        move || {
            let new_root = root.clone();
            App::new(new_root)
        },
        App::update,
        App::view,
    )
    .theme(|_: &App| Some(iced::Theme::GruvboxDark))
    .title("DDM Workbench")
    // .theme(app_theme)
    .window(window::Settings {
        size: Size::new(1200.0, 800.0),
        decorations: false,
        resizable: true,
        ..Default::default()
    }).subscription(App::subscription)
    .run()
}
