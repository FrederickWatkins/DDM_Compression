use std::path::Path;

pub fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(
            || path.as_os_str().to_string_lossy(),
            |name| name.to_string_lossy(),
        )
        .into_owned()
}
