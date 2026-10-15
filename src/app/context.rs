use std::path::PathBuf;

/// State shared by the top-level application and its view components.
#[derive(Debug, Default)]
pub(super) struct AppContext {
    pub document: DocumentContext,
    pub status: StatusContext,
}

/// State related to the currently open document or data file.
#[derive(Debug, Default)]
pub(super) struct DocumentContext {
    pub path: Option<PathBuf>,
}

impl DocumentContext {
    pub fn display_name(&self) -> &str {
        self.path
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("No file open")
    }
}

/// Text shown in the status bar at the bottom of the window.
#[derive(Debug)]
pub(super) struct StatusContext {
    pub message: String,
}

impl Default for StatusContext {
    fn default() -> Self {
        Self {
            message: "Ready".to_owned(),
        }
    }
}
