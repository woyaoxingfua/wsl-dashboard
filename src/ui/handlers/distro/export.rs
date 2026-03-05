use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use slint::ComponentHandle;
use crate::{AppWindow, AppState, i18n};

pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>, app_state: Arc<Mutex<AppState>>) {
    // Export process
    let ah = app_handle.clone();
    app.on_open_export_dialog(move |name| {
        info!("Operation: Open export dialog - {}", name);
        if let Some(app) = ah.upgrade() {
            if app.get_is_exporting() || app.get_is_cloning() || app.get_is_moving() || app.get_is_compacting() {
                app.set_current_message(i18n::t("dialog.operation_in_progress").into());
                app.set_show_message_dialog(true);
                return;
            }
            app.set_export_distro_name(name.into());
            // Default target directory to distro_location from settings
            let default_path = app.get_distro_location();
            app.set_export_target_path(default_path);
            app.set_export_error("".into());
            app.set_show_export_dialog(true);
        }
    });

    let ah = app_handle.clone();
    app.on_select_export_folder(move || {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(i18n::t("dialog.select_export_dir"))
            .pick_folder()
        {
            if let Some(app) = ah.upgrade() {
                app.set_export_target_path(path.display().to_string().into());
            }
        }
    });

    let ah = app_handle.clone();
    let as_ptr = app_state.clone();
    app.on_confirm_export(move |distro_source, target_path| {
        info!("Operation: Confirm export - Source: {}, Target Dir: {}", distro_source, target_path);
        let ah = match ah.upgrade() {
            Some(a) => a,
            None => return,
        };

        if target_path.is_empty() {
            ah.set_export_error(i18n::t("dialog.select_target_dir").into());
            return;
        }

        if ah.get_is_exporting() || ah.get_is_cloning() || ah.get_is_moving() || ah.get_is_compacting() {
            return;
        }

        ah.set_export_error("".into());
        ah.set_show_export_dialog(false);
        
        // Synchronously set exporting status to prevent double-click entry
        ah.set_is_exporting(true);
        
        let ah_clone = ah.as_weak();
        let as_ptr = as_ptr.clone();
        let distro_source = distro_source.to_string();
        let target_path = target_path.to_string();

        let _ = slint::spawn_local(async move {
            // Show exporting indicator using TaskStatusToast
            let stop_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
            if let Some(app) = ah_clone.upgrade() {
                let initial_msg = i18n::tr("operation.exporting_msg", &[distro_source.clone(), "0 MB".to_string()]);
                app.set_task_status_text(initial_msg.into());
                app.set_task_status_visible(true);
            }
            
            // Conflict resolution: add timestamp if file exists (e.g. Debian13.0.20260113141025.tar.gz)
            let mut filename = format!("{}.tar.gz", distro_source);
            let mut export_file = std::path::Path::new(&target_path).join(&filename);
            
            if export_file.exists() {
                let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
                filename = format!("{}.{}.tar.gz", distro_source, timestamp);
                export_file = std::path::Path::new(&target_path).join(&filename);
            }
            
            let export_file_str = export_file.to_string_lossy().to_string();
            
            // Start progress monitor
            super::spawn_file_size_monitor(
                ah_clone.clone(),
                export_file_str.clone(),
                distro_source.clone(),
                "operation.exporting_msg".into(),
                stop_signal.clone()
            );

            // Yield to event loop before long-running export
            tokio::task::yield_now().await;
            let result = {
                let dashboard = {
                    let state = as_ptr.lock().await;
                    state.wsl_dashboard.clone()
                };
                info!("Exporting distribution '{}' to '{}'...", distro_source, export_file_str);
                dashboard.export_distro(&distro_source, &export_file_str).await
            };

            // Stop monitor
            stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);

            if let Some(app) = ah_clone.upgrade() {
                // Hide exporting indicator
                app.set_task_status_visible(false);
                app.set_is_exporting(false);
                
                if result.success {
                    app.set_current_message(i18n::tr("dialog.export_success", &[distro_source.clone(), export_file_str.clone()]).into());
                } else {
                    let err = result.error.unwrap_or_else(|| i18n::t("dialog.error"));
                    app.set_current_message(i18n::tr("dialog.export_failed", &[err]).into());
                }
                app.set_show_message_dialog(true);
            }
        });
    });
}
