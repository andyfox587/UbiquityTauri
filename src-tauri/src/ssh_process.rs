/// SSH client using the system `ssh` command for maximum compatibility.
///
/// This is a fallback approach that shells out to the macOS OpenSSH binary
/// instead of using the russh library. This avoids signature verification
/// bugs in russh 0.48 with Dropbear SSH servers (used by UniFi APs).
///
/// Password authentication is handled via SSH_ASKPASS with a helper script.
use std::process::Stdio;
use tokio::process::Command;

const SSH_PORT: u16 = 22;
const DEFAULT_USERNAME: &str = "ubnt";
const DEFAULT_PASSWORD: &str = "ubnt";
const CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug)]
pub enum SshError {
    ConnectionRefused(String),
    ConnectionTimeout(String),
    AuthFailed(String),
    CommandFailed(String),
    Other(String),
}

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SshError::ConnectionRefused(msg) => write!(f, "Connection refused: {}", msg),
            SshError::ConnectionTimeout(msg) => write!(f, "Connection timeout: {}", msg),
            SshError::AuthFailed(msg) => write!(f, "Authentication failed: {}", msg),
            SshError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            SshError::Other(msg) => write!(f, "SSH error: {}", msg),
        }
    }
}

/// Execute set-inform on an AP via SSH using the system ssh command.
/// Uses factory-default credentials unless a custom password is provided.
pub async fn set_inform(
    ip: &str,
    inform_url: &str,
    custom_password: Option<&str>,
) -> Result<String, SshError> {
    let password = custom_password.unwrap_or(DEFAULT_PASSWORD);
    let command = format!("set-inform {}", inform_url);

    log::info!("Connecting to {} via system SSH (sshpass)...", ip);

    // First, check if sshpass is available
    let use_sshpass = Command::new("which")
        .arg("sshpass")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    let output = if use_sshpass {
        // Use sshpass for password auth
        log::info!("Using sshpass for authentication");
        tokio::time::timeout(
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS + 5),
            Command::new("sshpass")
                .arg("-p")
                .arg(password)
                .arg("ssh")
                .arg("-o").arg("StrictHostKeyChecking=no")
                .arg("-o").arg("UserKnownHostsFile=/dev/null")
                .arg("-o").arg(format!("ConnectTimeout={}", CONNECT_TIMEOUT_SECS))
                .arg("-o").arg("HostKeyAlgorithms=+ssh-rsa")
                .arg("-o").arg("PubkeyAcceptedAlgorithms=+ssh-rsa")
                .arg("-o").arg("PubkeyAuthentication=no")
                .arg("-p").arg(SSH_PORT.to_string())
                .arg(format!("{}@{}", DEFAULT_USERNAME, ip))
                .arg(&command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        )
        .await
        .map_err(|_| SshError::ConnectionTimeout(format!("Timed out connecting to {}", ip)))?
        .map_err(|e| SshError::Other(format!("Failed to run sshpass: {}", e)))?
    } else {
        // Fallback: use SSH_ASKPASS with a temporary script
        log::info!("sshpass not found, using SSH_ASKPASS method");

        // Create a temporary askpass script that echoes the password
        let askpass_dir = std::env::temp_dir();
        let askpass_path = askpass_dir.join("vivaspot_askpass.sh");
        let askpass_content = format!("#!/bin/sh\necho '{}'", password.replace('\'', "'\\''"));

        tokio::fs::write(&askpass_path, &askpass_content)
            .await
            .map_err(|e| SshError::Other(format!("Failed to create askpass script: {}", e)))?;

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(&askpass_path, perms)
                .map_err(|e| SshError::Other(format!("Failed to chmod askpass: {}", e)))?;
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS + 5),
            Command::new("ssh")
                .arg("-o").arg("StrictHostKeyChecking=no")
                .arg("-o").arg("UserKnownHostsFile=/dev/null")
                .arg("-o").arg(format!("ConnectTimeout={}", CONNECT_TIMEOUT_SECS))
                .arg("-o").arg("HostKeyAlgorithms=+ssh-rsa")
                .arg("-o").arg("PubkeyAcceptedAlgorithms=+ssh-rsa")
                .arg("-o").arg("PubkeyAuthentication=no")
                .arg("-o").arg("NumberOfPasswordPrompts=1")
                .arg("-p").arg(SSH_PORT.to_string())
                .arg(format!("{}@{}", DEFAULT_USERNAME, ip))
                .arg(&command)
                .env("SSH_ASKPASS", askpass_path.to_str().unwrap_or(""))
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("DISPLAY", ":0")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        )
        .await
        .map_err(|_| SshError::ConnectionTimeout(format!("Timed out connecting to {}", ip)))?
        .map_err(|e| SshError::Other(format!("Failed to run ssh: {}", e)))?;

        // Clean up askpass script
        let _ = tokio::fs::remove_file(&askpass_path).await;

        result
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::info!("SSH stdout: {}", stdout.trim());
    log::info!("SSH stderr: {}", stderr.trim());

    if !output.status.success() {
        let combined = format!("{}\n{}", stdout, stderr).trim().to_string();

        if combined.contains("Permission denied") || combined.contains("Authentication failed") {
            return Err(SshError::AuthFailed(format!(
                "Authentication failed for {} — password may have been changed from factory default",
                ip
            )));
        }
        if combined.contains("Connection refused") {
            return Err(SshError::ConnectionRefused(format!(
                "Connection refused at {}", ip
            )));
        }
        if combined.contains("timed out") || combined.contains("Connection timeout") {
            return Err(SshError::ConnectionTimeout(format!(
                "Timed out connecting to {}", ip
            )));
        }

        return Err(SshError::Other(format!(
            "Failed to connect to {}: {}",
            ip,
            combined
        )));
    }

    // Any output without "error" is generally success
    if stdout.to_lowercase().contains("error") && !stdout.to_lowercase().contains("inform") {
        return Err(SshError::CommandFailed(format!(
            "set-inform returned an error: {}",
            stdout.trim()
        )));
    }

    Ok(stdout.trim().to_string())
}
