mod api;
mod discovery;
mod ssh;

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
#[tauri::command]
async fn adopt_device(
    ip: String,
    inform_url: String,
    custom_password: Option<String>,
) -> Result<AdoptResult, String> {
    let password_ref = custom_password.as_deref();

    let output = ssh::set_inform(&ip, &inform_url, password_ref)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AdoptResult {
        success: true,
        output,
    })
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
