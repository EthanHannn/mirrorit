pub mod adapters;
pub mod commands;
pub mod domain;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::flutter_pub::FlutterPubPreviewStore::default())
        .manage(commands::npm::NpmPreviewStore::default())
        .manage(commands::maven::MavenPreviewStore::default())
        .invoke_handler(tauri::generate_handler![
            commands::cargo::scan_cargo,
            commands::docker::scan_docker,
            commands::go::scan_go,
            commands::flutter_pub::scan_flutter_pub,
            commands::flutter_pub::preview_flutter_pub_hosted_update,
            commands::flutter_pub::apply_flutter_pub_preview,
            commands::flutter_pub::rollback_flutter_pub_snapshot,
            commands::maven::scan_maven,
            commands::maven::preview_maven_mirror_update,
            commands::maven::apply_maven_preview,
            commands::maven::rollback_maven_snapshot,
            commands::npm::scan_npm,
            commands::npm::preview_npm_profile,
            commands::npm::apply_npm_preview,
            commands::npm::rollback_npm_snapshot,
            commands::npm::check_npm_health,
            commands::pnpm::scan_pnpm,
            commands::yarn::scan_yarn,
            commands::pip::scan_pip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
