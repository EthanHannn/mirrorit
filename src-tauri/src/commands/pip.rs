use crate::adapters::{pip::PipAdapter, ConfigAdapter};
use crate::domain::{ReadResult, ToolContext};

#[tauri::command]
pub fn scan_pip() -> Result<ReadResult, String> {
    PipAdapter::from_system()
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)
}
