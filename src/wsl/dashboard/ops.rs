use tokio::time::Duration;
use tracing::{info, warn, debug};
use crate::wsl::models::WslCommandResult;
use super::WslDashboard;

impl WslDashboard {
    pub async fn start_distro(&self, name: &str) -> WslCommandResult<String> {
        self.increment_manual_operation();
        let result = self.executor.start_distro(name).await;
        if result.success {
            info!("WSL distro '{}' startup command executed, waiting for status update", name);
            let _ = self.refresh_distros().await;
            
            let manager_clone = self.clone();
            let name_clone = name.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(3)).await;
                info!("Delayed refresh of WSL distro '{}' status after startup", name_clone);
                let _ = manager_clone.refresh_distros().await;
                manager_clone.decrement_manual_operation();
            });
        } else {
            self.decrement_manual_operation();
        }
        result
    }

    pub async fn stop_distro(&self, name: &str) -> WslCommandResult<String> {
        self.increment_manual_operation();
        info!("Calling executor.stop_distro for '{}'", name);
        let result = self.executor.stop_distro(name).await;
        info!("Executor returned from stop_distro for '{}' (success: {})", name, result.success);

        if result.success {
            info!("WSL distro '{}' termination command executed, waiting for status update", name);
            let _ = self.refresh_distros().await;
            info!("Immediate refresh after stop completed for '{}'", name);
            
            let manager_clone = self.clone();
            let name_clone = name.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(3)).await;
                info!("Delayed refresh of WSL distro '{}' status after termination", name_clone);
                let _ = manager_clone.refresh_distros().await;
                manager_clone.decrement_manual_operation();
            });
        } else {
            self.decrement_manual_operation();
        }
        result
    }

    pub async fn restart_distro(&self, name: &str) -> WslCommandResult<String> {
        info!("WSL distro '{}' restart initiated", name);
        let stop_result = self.stop_distro(name).await;
        if !stop_result.success {
            warn!("Stop failed during restart for '{}', aborting restart", name);
            return stop_result;
        }
        info!("Stop successful for '{}', waiting 4s before start...", name);
        tokio::time::sleep(Duration::from_secs(4)).await;
        info!("Wait complete for '{}', initiating start...", name);
        self.start_distro(name).await
    }

    pub async fn shutdown_wsl(&self) -> WslCommandResult<String> {
        self.increment_manual_operation();
        info!("Initiating WSL system shutdown");
        let result = self.executor.shutdown_wsl().await;
        if result.success {
            let _ = self.refresh_distros().await;
        }
        self.decrement_manual_operation();
        result
    }

    pub async fn delete_distro(&self, config_manager: &crate::config::ConfigManager, name: &str) -> WslCommandResult<String> {
        let _heavy_lock = self.heavy_op_lock.lock().await;
        self.increment_manual_operation();
        
        let self_clone = self.clone();
        let _op_guard = scopeguard::guard((), |_| {
            self_clone.decrement_manual_operation();
        });

        warn!("Initiating deletion of WSL distro '{}' (irreversible operation)", name);
        let result = self.executor.delete_distro(config_manager, name).await;
        
        if result.success {
            // Immediate local update to make UI responsive
            {
                let mut distros = self.distros.lock().await;
                let old_len = distros.len();
                distros.retain(|d| d.name != name);
                if distros.len() < old_len {
                    debug!("Manually removed '{}' from local cache, notifying UI", name);
                    self.state_changed.notify_one();
                }
            }
            // Full refresh is now deferred to the background monitor once manual_operation drops to 0
        }

        // Lock is released here at end of scope
        result
    }

    pub async fn export_distro(&self, name: &str, file_path: &str) -> WslCommandResult<String> {
        let _heavy_lock = self.heavy_op_lock.lock().await;
        self.increment_manual_operation();
        let result = self.executor.export_distro(name, file_path).await;
        self.decrement_manual_operation();
        result
    }

    pub async fn import_distro(&self, name: &str, install_location: &str, file_path: &str) -> WslCommandResult<String> {
        let _heavy_lock = self.heavy_op_lock.lock().await;
        self.increment_manual_operation();
        let result = self.executor.import_distro(name, install_location, file_path).await;
        if result.success {
            let _ = self.refresh_distros().await;
        }
        self.decrement_manual_operation();
        result
    }

    pub async fn move_distro(&self, name: &str, new_path: &str) -> WslCommandResult<String> {
        let _heavy_lock = self.heavy_op_lock.lock().await;
        self.increment_manual_operation();
        let result = self.executor.move_distro(name, new_path).await;
        if result.success {
            let _ = self.refresh_distros().await;
        }
        self.decrement_manual_operation();
        result
    }

    pub async fn compact_distro(&self, name: &str) -> WslCommandResult<String> {
        let _heavy_lock = self.heavy_op_lock.lock().await;
        self.increment_manual_operation();
        let result = self.executor.compact_distro(name).await;
        if result.success {
            let _ = self.refresh_distros().await;
        }
        self.decrement_manual_operation();
        result
    }

    pub async fn open_distro_bashrc(&self, name: &str) -> WslCommandResult<String> {
        self.executor.open_distro_folder_path(name, "~").await
    }

    #[allow(dead_code)]
    pub async fn open_distro_folder(&self, distro_name: &str) -> WslCommandResult<String> {
        self.executor.open_distro_folder(distro_name).await
    }
}
