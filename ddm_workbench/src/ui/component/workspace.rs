use iced::{
    Color, Element, Length, Task,
    application::UpdateFn,
    widget::{container, text},
};
use iced_aw::{TabLabel, tabs::Tabs};
use slotmap::{DefaultKey, SlotMap};

use crate::{L, XL};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Message {
    SelectTab(DefaultKey),
    NewTab(Tab),
    CloseTab(DefaultKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TabKind {
    DDMPlayer,
    DDMInspector,
    Plotter,
}

impl TabKind {
    pub fn view(&self) -> Element<'_, Message> {
        match self {
            TabKind::DDMPlayer => text("Hello!!!!")
                .color(Color::from_rgb(0.8, 0.2, 0.2))
                .into(),
            TabKind::DDMInspector => text("World!!!!")
                .color(Color::from_rgb(0.2, 0.2, 0.8))
                .into(),
            TabKind::Plotter => text("Plotter!!!!")
                .color(Color::from_rgb(0.2, 0.8, 0.2))
                .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tab {
    pub name: String,
    pub kind: TabKind,
}

impl Tab {
    pub fn view(&self) -> Element<'_, Message> {
        self.kind.view()
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    active_tab: Option<DefaultKey>,
    tabs: SlotMap<DefaultKey, Tab>,
}

impl Workspace {
    pub fn new() -> Self {
        let active_tab = None;
        let mut tabs = SlotMap::new();
        tabs.insert(Tab {
            name: "hello".to_owned(),
            kind: TabKind::DDMPlayer,
        });
        tabs.insert(Tab {
            name: "world".to_owned(),
            kind: TabKind::DDMInspector,
        });
        Self { active_tab, tabs }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTab(active_tab) => {
                self.active_tab = Some(active_tab);
                Task::none()
            }
            Message::NewTab(tab) => {
                self.active_tab = Some(self.tabs.insert(tab));
                Task::none()
            }
            Message::CloseTab(tab) => {
                self.tabs.remove(tab);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut tab_manager = Tabs::new(Message::SelectTab)
            .width(Length::Fill)
            .height(Length::Fill)
            .tab_bar_height(Length::Fixed(L))
            .close_size(XL)
            .tab_bar_width(Length::Fixed(200.0 * self.tabs.len() as f32))
            .on_close(Message::CloseTab);
        for tab in &self.tabs {
            tab_manager = tab_manager.push(tab.0, TabLabel::Text(tab.1.name.clone()), tab.1.view());
        }
        if let Some(active_tab) = self.active_tab {
            tab_manager = tab_manager.set_active_tab(&active_tab);
        }
        tab_manager.into()
    }
}
