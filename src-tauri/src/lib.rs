mod api;
mod discovery;
mod ssh;
mod ssh_process;

use serde::Serialize;

// ============================================================
// Tauri command return types
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateCodeResult {
    inform_url: String,
    site_id: String,
    site_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    devices: Vec<discovery::DiscoveredDevice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdoptResult {
    success: bool,
    output: String,
}

// ============================================================
// Tauri commands — called from the React frontend via invoke()
// ============================================================

/// Validate a setup code against the VivaSpot API.
/// Returns inform URL and site metadata.
#[tauri::command]
async fn validate_code(code: String) -> Result<ValidateCodeResult, String> {
    let result = api::validate_setup_code(&code)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ValidateCodeResult {
        inform_url: result.inform_url,
        site_id: result.site_id,
        site_name: result.site_name,
    })
}

/// Scan the local network for UniFi devices via UDP broadcast.
#[tauri::command]
async fn scan_devices() -> Result<ScanResult, String> {
    // Run the blocking UDP scan on a separate thread
    let devices = tokio::task::spawn_blocking(discovery::scan_network)
        .await
        .map_err(|e| format!("Scan task failed: {}", e))?
        .map_err(|e| e)?;

    Ok(ScanResult { devices })
}

/// Execute set-inform on an AP via SSH.
/// Uses the system ssh command (via ssh_process) for maximum compatibility
/// with Dropbear SSH on UniFi APs. Falls back to russh library if that fails.
#[tauri::command]
async fn adopt_device(
    ip: String,
    inform_url: String,
    custom_password: Option<String>,
) -> Result<AdoptResult, String> {
    let password_ref = custom_password.as_deref();

    // Try system SSH first (uses macOS OpenSSH via expect, proven compatible with Dropbear)
    log::info!("Attempting SSH via system expect command...");
    match ssh_process::set_inform(&ip, &inform_url, password_ref).await {
        Ok(output) => {
            log::info!("System SSH succeeded");
            return Ok(AdoptResult {
                success: true,
                output,
            });
        }
        Err(e) => {
            let err_str = e.to_string();
            log::warn!("System SSH failed: {}", err_str);

            // If it's an auth failure, don't bother with russh — report it directly
            if matches!(e, ssh_process::SshError::AuthFailed(_)) {
                return Err(err_str);
            }

            // For other failures (e.g. expect not found), try russh as fallback
            log::info!("Falling back to russh library...");
            match ssh::set_inform(&ip, &inform_url, password_ref).await {
                Ok(output) => {
                    return Ok(AdoptResult {
                        success: true,
                        output,
                    });
                }
                Err(russh_err) => {
                    // Return whichever error is more informative
                    let russh_str = russh_err.to_string();
                    log::warn!("russh also failed: {}", russh_str);
                    return Err(format!(
                        "SSH error: Failed to connect to {}: {}",
                        ip, err_str
                    ));
                }
            }
        }
    }
}

/// Return the app version for display in the UI.
#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ============================================================
// App entry point
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            validate_code,
            scan_devices,
            adopt_device,
            get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
