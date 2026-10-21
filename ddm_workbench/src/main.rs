//! DDM workspace
use iced::{
    Alignment, Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
    widget::{Space, button, canvas, column, container, mouse_area, progress_bar, row, svg, text},
    window,
};
use iced_aw::{
    TabLabel,
    menu::{self, Item, Menu, MenuBar},
    tabs::Tabs,
};
use plotters::{
    prelude::{IntoFont, LineSeries, RGBColor},
    style::Color as PlottersColor,
};
use plotters_iced2::{Chart, ChartBuilder, ChartWidget, DrawingBackend};
use tracing_subscriber::filter::LevelFilter;

// -----------------------------------------------------------------------------
// Embedded SVG assets
// -----------------------------------------------------------------------------

const LOGO: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo.svg"));

const MINIMIZE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <path d="M3 8h10" fill="none" stroke="#e9e9f2"
        stroke-width="1.5" stroke-linecap="round"/>
</svg>
"##;

const MAXIMIZE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <rect x="3.25" y="3.25" width="9.5" height="9.5"
        fill="none" stroke="#e9e9f2" stroke-width="1.4"
        rx="1"/>
</svg>
"##;

const CLOSE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <path d="M4 4l8 8M12 4l-8 8"
        fill="none" stroke="#e9e9f2"
        stroke-width="1.5" stroke-linecap="round"/>
</svg>
"##;

// -----------------------------------------------------------------------------
// Theme
// -----------------------------------------------------------------------------

const BACKGROUND: Color = Color::from_rgb8(22, 23, 32);
const TEXT_COLOR: Color = Color::from_rgb8(233, 233, 242);
const ACCENT: Color = Color::from_rgb8(185, 146, 255);

fn app_theme(_: &App) -> Theme {
    Theme::custom(
        "DDM Workbench",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT_COLOR,
            primary: ACCENT,
            success: Color::from_rgb8(119, 222, 186),
            warning: Color::from_rgb8(255, 197, 120),
            danger: Color::from_rgb8(255, 111, 139),
        },
    )
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .pretty()
        .compact()
        .with_target(true)
        .with_file(false)
        .init();
    iced::application(App::new, App::update, App::view)
        .title("DDM Workbench")
        .theme(app_theme)
        .window(window::Settings {
            size: Size::new(1200.0, 800.0),
            min_size: Some(Size::new(720.0, 480.0)),
            decorations: false,
            resizable: true,
            ..Default::default()
        })
        .run()
}

// -----------------------------------------------------------------------------
// Application state
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Plot,
    Canvas,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(Tab),
    WindowId(Option<window::Id>),

    OpenL0b,
    OpenRecent,
    ExportSvg,
    ExportPng,
    ExportCsv,
    Quit,

    DragWindow,
    Minimize,
    ToggleMaximize,
    Close,
}

struct App {
    active_tab: Tab,
    status: String,
    parabola: Parabola,
    window_id: Option<window::Id>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                active_tab: Tab::Plot,
                status: "Ready".into(),
                parabola: Parabola,
                window_id: None,
            },
            window::latest().map(Message::WindowId),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Task::none()
            }

            // Connect these to a file picker and your actual file operations.
            Message::OpenL0b => {
                self.status = "Open L0B selected".into();
                Task::none()
            }

            Message::OpenRecent => {
                self.status = "Recent files selected".into();
                Task::none()
            }

            Message::ExportSvg => {
                self.status = "Export plot as SVG selected".into();
                Task::none()
            }

            Message::ExportPng => {
                self.status = "Export plot as PNG selected".into();
                Task::none()
            }

            Message::ExportCsv => {
                self.status = "Export data as CSV selected".into();
                Task::none()
            }
            Message::WindowId(id) => {
                self.window_id = id;
                Task::none()
            }

            Message::Quit | Message::Close => match self.window_id {
                Some(id) => window::close(id),
                None => Task::none(),
            },

            Message::Minimize => match self.window_id {
                Some(id) => window::minimize(id, true),
                None => Task::none(),
            },

            Message::ToggleMaximize => match self.window_id {
                Some(id) => window::toggle_maximize(id),
                None => Task::none(),
            },

            Message::DragWindow => match self.window_id {
                Some(id) => window::drag(id),
                None => Task::none(),
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // ---------------------------------------------------------------------
        // File -> Open submenu
        // ---------------------------------------------------------------------

        let open_menu = Menu::new(
            vec![
                Item::new(button("L0B file…").on_press(Message::OpenL0b)),
                Item::new(button("Recent files…").on_press(Message::OpenRecent)),
            ]
            .into(),
        )
        .max_width(200.0)
        .offset(6.0);

        // ---------------------------------------------------------------------
        // File -> Export submenu
        // ---------------------------------------------------------------------

        let export_menu = Menu::new(
            vec![
                Item::new(button("Plot as SVG").on_press(Message::ExportSvg)),
                Item::new(button("Plot as PNG").on_press(Message::ExportPng)),
                Item::new(button("Data as CSV").on_press(Message::ExportCsv)),
            ]
            .into(),
        )
        .max_width(200.0)
        .offset(6.0);

        // ---------------------------------------------------------------------
        // File menu bar
        // ---------------------------------------------------------------------

        let file_menu = Menu::new(
            vec![
                Item::with_menu(button("Open"), open_menu),
                Item::with_menu(button("Export"), export_menu),
                Item::new(button("Quit").on_press(Message::Quit)),
            ]
            .into(),
        )
        .max_width(200.0)
        .offset(6.0);

        let menu_bar = MenuBar::new(vec![Item::with_menu(button("File"), file_menu)])
            .draw_path(menu::DrawPath::Backdrop);

        // ---------------------------------------------------------------------
        // Custom title bar
        // ---------------------------------------------------------------------

        let title_bar = container(
            row![
                svg(svg::Handle::from_memory(LOGO))
                    .width(Length::Fixed(32.0))
                    .height(Length::Fixed(32.0)),
                text("DDM WORKBENCH").size(14).font(Font::MONOSPACE),
                menu_bar,
                progress_bar(0.0..=1.0, 0.6),
                // The remaining space acts as the native window drag handle.
                mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(36.0)))
                    .on_press(Message::DragWindow),
                button(
                    svg(svg::Handle::from_memory(MINIMIZE_ICON))
                        .width(Length::Fixed(14.0))
                        .height(Length::Fixed(14.0))
                )
                .padding(8)
                .on_press(Message::Minimize),
                button(
                    svg(svg::Handle::from_memory(MAXIMIZE_ICON))
                        .width(Length::Fixed(14.0))
                        .height(Length::Fixed(14.0))
                )
                .padding(8)
                .on_press(Message::ToggleMaximize),
                button(
                    svg(svg::Handle::from_memory(CLOSE_ICON))
                        .width(Length::Fixed(14.0))
                        .height(Length::Fixed(14.0))
                )
                .padding(8)
                .on_press(Message::Close),
            ]
            .align_y(Alignment::Center)
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(8);

        // ---------------------------------------------------------------------
        // Plot tab: y = x²
        // ---------------------------------------------------------------------

        let plot_tab = ChartWidget::new(&self.parabola)
            .width(Length::Fill)
            .height(Length::Fill);

        // ---------------------------------------------------------------------
        // Canvas tab: primitive shapes
        // ---------------------------------------------------------------------

        let canvas_tab = container(
            column![
                text("Canvas primitives").size(20),
                text("Rectangle · circle · triangle · line").size(13),
                canvas::Canvas::new(PrimitiveScene)
                    .width(Length::Fill)
                    .height(Length::Fill),
            ]
            .spacing(12),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(18);

        // ---------------------------------------------------------------------
        // Main tab widget
        // ---------------------------------------------------------------------

        let tabs = Tabs::new(Message::TabSelected)
            .push(Tab::Plot, TabLabel::Text("Plotter".into()), plot_tab)
            .push(Tab::Canvas, TabLabel::Text("Canvas".into()), canvas_tab)
            .set_active_tab(&self.active_tab)
            .width(Length::Fill)
            .height(Length::Fill);

        // ---------------------------------------------------------------------
        // Status bar and root layout
        // ---------------------------------------------------------------------

        let status_bar = container(text(&self.status).size(12).font(Font::MONOSPACE))
            .width(Length::Fill)
            .padding(10);

        container(
            column![
                title_bar,
                container(tabs)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(12),
                status_bar,
            ]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

// -----------------------------------------------------------------------------
// Plotters chart
// -----------------------------------------------------------------------------

struct Parabola;

impl Chart<Message> for Parabola {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let caption_style = ("sans-serif", 22)
            .into_font()
            .color(&RGBColor(233, 233, 242));

        let Ok(mut chart) = builder
            .caption("y = x²", caption_style)
            .margin(18)
            .x_label_area_size(34)
            .y_label_area_size(44)
            .build_cartesian_2d(-5.0_f32..5.0_f32, 0.0_f32..25.0_f32)
        else {
            return;
        };

        let _ = chart
            .configure_mesh()
            .x_desc("x")
            .y_desc("y")
            .label_style(
                ("sans-serif", 13)
                    .into_font()
                    .color(&RGBColor(190, 190, 210)),
            )
            .axis_style(RGBColor(100, 100, 125))
            .light_line_style(RGBColor(75, 75, 100).mix(0.25))
            .draw();

        let points = (-100..=100).map(|i| {
            let x = i as f32 / 20.0;
            (x, x * x)
        });

        let _ = chart.draw_series(LineSeries::new(points, &RGBColor(185, 146, 255)));
    }
}

// -----------------------------------------------------------------------------
// Iced canvas drawing
// -----------------------------------------------------------------------------

struct PrimitiveScene;

impl canvas::Program<Message> for PrimitiveScene {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let width = bounds.width;
        let height = bounds.height;

        // Canvas background.
        frame.fill(
            &canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size()),
            Color::from_rgb8(25, 26, 38),
        );

        // Rectangle.
        let rectangle = canvas::Path::rectangle(
            Point::new(width * 0.10, height * 0.22),
            Size::new(width * 0.22, height * 0.30),
        );

        frame.fill(&rectangle, Color::from_rgb8(90, 65, 145));

        frame.stroke(
            &rectangle,
            canvas::Stroke::default().with_color(ACCENT).with_width(2.0),
        );

        // Circle.
        let circle = canvas::Path::circle(
            Point::new(width * 0.53, height * 0.37),
            height.min(width) * 0.11,
        );

        frame.fill(&circle, Color::from_rgb8(44, 120, 150));

        frame.stroke(
            &circle,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(105, 210, 235))
                .with_width(2.0),
        );

        // Triangle.
        let triangle = canvas::Path::new(|path| {
            path.move_to(Point::new(width * 0.75, height * 0.22));
            path.line_to(Point::new(width * 0.66, height * 0.53));
            path.line_to(Point::new(width * 0.87, height * 0.53));
            path.close();
        });

        frame.fill(&triangle, Color::from_rgb8(155, 75, 105));

        frame.stroke(
            &triangle,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(255, 130, 165))
                .with_width(2.0),
        );

        // Diagonal line.
        let line = canvas::Path::line(
            Point::new(width * 0.10, height * 0.75),
            Point::new(width * 0.87, height * 0.75),
        );

        frame.stroke(
            &line,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(119, 222, 186))
                .with_width(3.0),
        );

        vec![frame.into_geometry()]
    }
}
