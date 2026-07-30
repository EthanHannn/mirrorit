use crate::adapters::{maven::MavenAdapter, ConfigAdapter};
use crate::domain::{ReadResult, ToolContext};
#[tauri::command]
pub fn scan_maven() -> Result<ReadResult, String> {
    MavenAdapter::from_system()
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)
}
