pub mod cargo;
pub mod docker;
pub mod flutter_pub;
pub mod go;
pub mod maven;
pub mod npm;
pub mod pip;
pub mod pnpm;
mod preview;
pub mod profiles;
pub mod yarn;

async fn run_read<T: Send + 'static>(
    action: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    // 磁盘、网络与子进程等待不占用窗口事件线程。
    tauri::async_runtime::spawn_blocking(action)
        .await
        .map_err(|_| "后台读取未完成，请重试。".to_owned())?
}
