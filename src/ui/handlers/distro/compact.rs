use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use slint::{ComponentHandle, Model};
use crate::{AppWindow, AppState, i18n};
use crate::ui::data::refresh_distros_ui;

pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>, app_state: Arc<Mutex<AppState>>) {
    let ah = app_handle.clone();
    let as_ptr = app_state.clone();
    app.on_compact_distro(move |name| {
        info!("Operation: Compact disk - {}", name);
        let ah = match ah.upgrade() {
            Some(a) => a,
            None => return,
        };

        if ah.get_is_installing()
            || ah.get_is_exporting()
            || ah.get_is_cloning()
            || ah.get_is_moving()
            || ah.get_is_compacting()
        {
            ah.set_current_message(i18n::t("dialog.operation_in_progress").into());
            ah.set_show_message_dialog(true);
            return;
        }

        let mut is_wsl2 = false;
        let distros = ah.get_distros();
        for i in 0..distros.row_count() {
            if let Some(d) = distros.row_data(i) {
                if d.name == name {
                    is_wsl2 = d.version.to_string() == "2";
                    break;
                }
            }
        }

        if !is_wsl2 {
            ah.set_current_message(i18n::t("dialog.compact_wsl2_only").into());
            ah.set_show_message_dialog(true);
            return;
        }

        let distro_name = name.to_string();
        ah.set_is_compacting(true);
        ah.set_task_status_text(i18n::tr("operation.compacting", &[distro_name.clone()]).into());
        ah.set_task_status_visible(true);

        let ah_weak = ah.as_weak();
        let as_ptr = as_ptr.clone();

        let _ = slint::spawn_local(async move {
            let dashboard = {
                let state = as_ptr.lock().await;
                state.wsl_dashboard.clone()
            };

            // Compact needs the VHDX handle to be released, so force a full WSL shutdown first.
            let _ = dashboard.shutdown_wsl().await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let result = dashboard.compact_distro(&distro_name).await;

            if let Some(app) = ah_weak.upgrade() {
                app.set_is_compacting(false);
                app.set_task_status_visible(false);
                if result.success {
                    app.set_current_message(i18n::tr("dialog.compact_success", &[distro_name.clone()]).into());
                } else {
                    let err = result.error.unwrap_or_else(|| i18n::t("dialog.error"));
                    app.set_current_message(i18n::tr("dialog.compact_failed", &[err]).into());
                }
                app.set_show_message_dialog(true);
            }

            refresh_distros_ui(ah_weak.clone(), as_ptr.clone()).await;
        });
    });
}
