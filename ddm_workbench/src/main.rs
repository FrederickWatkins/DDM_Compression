//! DDM workbench
use ddm_workbench::app::App;
use iced::{Size, window};
use tracing_subscriber::EnvFilter;

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
    .theme(|_: &App| Some(iced::Theme::CatppuccinMocha))
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
