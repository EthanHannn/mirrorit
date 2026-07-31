use crate::adapters::{go::GoAdapter, ConfigAdapter};
use crate::domain::{ReadResult, ToolContext};

#[tauri::command]
pub fn scan_go() -> Result<ReadResult, String> {
    GoAdapter::from_system()
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)
}
