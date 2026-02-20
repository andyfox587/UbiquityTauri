/// SSH client using the system `ssh` command for maximum compatibility.
///
/// This shells out to the macOS OpenSSH binary instead of using the russh
/// library, avoiding signature verification bugs in russh 0.48 with
/// Dropbear SSH servers (used by UniFi APs).
///
/// Password authentication is handled via an `expect` script (macOS ships
/// with expect pre-installed as part of the developer tools / Tcl).
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

/// Execute set-inform on an AP via SSH using the system ssh + expect.
/// Uses factory-default credentials unless a custom password is provided.
pub async fn set_inform(
    ip: &str,
    inform_url: &str,
    custom_password: Option<&str>,
) -> Result<String, SshError> {
    let password = custom_password.unwrap_or(DEFAULT_PASSWORD);
    let ssh_command = format!("set-inform {}", inform_url);

    log::info!("Connecting to {} via system SSH (expect)...", ip);

    // Build an expect script that handles SSH password authentication.
    // macOS ships with expect as part of Tcl (/usr/bin/expect).
    let expect_script = format!(
        r#"#!/usr/bin/expect -f
set timeout {timeout}
spawn ssh -o StrictHostKeyChecking=no \
    -o UserKnownHostsFile=/dev/null \
    -o PubkeyAuthentication=no \
    -o HostKeyAlgorithms=+ssh-rsa \
    -o PubkeyAcceptedAlgorithms=+ssh-rsa \
    -o ConnectTimeout={timeout} \
    -p {port} \
    {user}@{host} "{cmd}"

expect {{
    "assword:" {{
        send "{pass}\r"
        expect {{
            "assword:" {{
                puts stderr "AUTH_FAILED"
                exit 1
            }}
            eof {{
                catch wait result
                exit [lindex $result 3]
            }}
        }}
    }}
    "Connection refused" {{
        puts stderr "CONNECTION_REFUSED"
        exit 1
    }}
    timeout {{
        puts stderr "CONNECTION_TIMEOUT"
        exit 1
    }}
    eof {{
        catch wait result
        exit [lindex $result 3]
    }}
}}
"#,
        timeout = CONNECT_TIMEOUT_SECS,
        port = SSH_PORT,
        user = DEFAULT_USERNAME,
        host = ip,
        cmd = ssh_command.replace('"', r#"\""#),
        pass = password.replace('\\', r"\\").replace('"', r#"\""#),
    );

    // Write the expect script to a temp file
    let script_dir = std::env::temp_dir();
    let script_path = script_dir.join("vivaspot_ssh.exp");

    tokio::fs::write(&script_path, &expect_script)
        .await
        .map_err(|e| SshError::Other(format!("Failed to write expect script: {}", e)))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(&script_path, perms)
            .map_err(|e| SshError::Other(format!("Failed to chmod script: {}", e)))?;
    }

    // Run the expect script
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS + 10),
        Command::new("expect")
            .arg(script_path.to_str().unwrap_or(""))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| SshError::ConnectionTimeout(format!("Timed out connecting to {}", ip)))?
    .map_err(|e| SshError::Other(format!("Failed to run expect: {}", e)))?;

    // Clean up
    let _ = tokio::fs::remove_file(&script_path).await;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    log::info!("expect stdout: {}", stdout.trim());
    log::info!("expect stderr: {}", stderr.trim());

    if !output.status.success() {
        if stderr.contains("AUTH_FAILED") || stdout.contains("Permission denied") {
            return Err(SshError::AuthFailed(format!(
                "Authentication failed for {} — password may have been changed from factory default",
                ip,
            )));
        }
        if stderr.contains("CONNECTION_REFUSED") || stdout.contains("Connection refused") {
            return Err(SshError::ConnectionRefused(format!(
                "Connection refused at {}",
                ip,
            )));
        }
        if stderr.contains("CONNECTION_TIMEOUT") || stdout.contains("timed out") {
            return Err(SshError::ConnectionTimeout(format!(
                "Timed out connecting to {}",
                ip,
            )));
        }

        let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
        return Err(SshError::Other(format!(
            "Failed to connect to {}: {}",
            ip,
            combined.trim(),
        )));
    }

    // Filter out expect's echoed output (the spawn line, password prompt, etc.)
    // The actual set-inform output is what comes after the password was sent.
    let useful_output: String = stdout
        .lines()
        .filter(|line| {
            !line.starts_with("spawn ")
                && !line.contains("assword:")
                && !line.contains("Warning: Permanently added")
                && !line.trim().is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n");

    log::info!("set-inform result: {}", useful_output.trim());

    // Check for errors in the output
    if useful_output.to_lowercase().contains("error")
        && !useful_output.to_lowercase().contains("inform")
    {
        return Err(SshError::CommandFailed(format!(
            "set-inform returned an error: {}",
            useful_output.trim(),
        )));
    }

    Ok(useful_output.trim().to_string())
}
