//! DDM TUI viewer

mod app;
pub use app::App;

mod ui;
pub use ui::ui;

const WIDTH: usize = 20;
const HEIGHT: usize = 52;

type L0bFileData = ddm_app::L0bFileData<WIDTH, HEIGHT>;
type DDMFrame = ddm_core::l0b::DDMFrame<WIDTH, HEIGHT>;
