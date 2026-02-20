/// SSH client for executing set-inform on UniFi APs.
///
/// Flow (see design doc §4.5.4):
/// 1. Connect to AP's IP on port 22
/// 2. Authenticate with ubnt/ubnt (factory defaults) or custom password
/// 3. Execute: set-inform <inform_url>
/// 4. Parse response to confirm success
/// 5. Disconnect
use std::borrow::Cow;
use std::sync::Arc;
use russh::*;
use russh::kex;
use russh::Preferred;
use russh_keys::ssh_key::{Algorithm, EcdsaCurve};

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

struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept all host keys (these are factory-reset APs on local network)
        Ok(true)
    }
}

/// Execute set-inform on an AP via SSH.
/// Uses factory-default credentials unless a custom password is provided.
pub async fn set_inform(
    ip: &str,
    inform_url: &str,
    custom_password: Option<&str>,
) -> Result<String, SshError> {
    let password = custom_password.unwrap_or(DEFAULT_PASSWORD);

    log::info!("Connecting to {} via SSH...", ip);

    // Configure SSH for compatibility with UniFi APs (Dropbear SSH).
    //
    // IMPORTANT: russh 0.48 has a bug where it always verifies RSA signatures
    // using SHA-1 (via sig_workaround.rs), regardless of the negotiated host key
    // algorithm. If rsa-sha2-256 is negotiated, the server signs with SHA-256
    // but russh verifies with SHA-1 → "Wrong server signature".
    //
    // Workaround: only offer ssh-rsa (SHA-1) for host keys so both sides
    // agree on SHA-1 signing. Also include legacy kex algorithms.
    let mut config = client::Config::default();
    config.preferred = Preferred {
        kex: Cow::Owned(vec![
            kex::CURVE25519,
            kex::CURVE25519_PRE_RFC_8731,
            kex::DH_G16_SHA512,
            kex::DH_G14_SHA256,
            kex::DH_G14_SHA1,
            kex::DH_G1_SHA1,
        ]),
        key: Cow::Owned(vec![
            // ONLY offer ssh-rsa (SHA-1) for RSA keys due to the russh bug above.
            // Do NOT include rsa-sha2-256 or rsa-sha2-512.
            Algorithm::Rsa { hash: None },
            Algorithm::Ed25519,
            Algorithm::Ecdsa { curve: EcdsaCurve::NistP256 },
            Algorithm::Ecdsa { curve: EcdsaCurve::NistP384 },
            Algorithm::Ecdsa { curve: EcdsaCurve::NistP521 },
        ]),
        ..config.preferred
    };
    let config = Arc::new(config);

    let addr = format!("{}:{}", ip, SSH_PORT);

    let mut handle = tokio::time::timeout(
        std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
        client::connect(config, &addr, ClientHandler),
    )
    .await
    .map_err(|_| SshError::ConnectionTimeout(format!("Timed out connecting to {}", ip)))?
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("refused") {
            SshError::ConnectionRefused(format!("Connection refused at {}", ip))
        } else {
            SshError::Other(format!("Failed to connect to {}: {}", ip, msg))
        }
    })?;

    log::info!("Connected to {}, authenticating...", ip);

    let auth_result = handle
        .authenticate_password(DEFAULT_USERNAME, password)
        .await
        .map_err(|e| SshError::Other(format!("Auth error: {}", e)))?;

    if !auth_result {
        return Err(SshError::AuthFailed(format!(
            "Authentication failed for {} — password may have been changed from factory default",
            ip
        )));
    }

    log::info!("Authenticated to {}, executing set-inform...", ip);

    let command = format!("set-inform {}", inform_url);

    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| SshError::Other(format!("Failed to open channel: {}", e)))?;

    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|e| SshError::CommandFailed(format!("Failed to execute command: {}", e)))?;

    // Read response
    let mut output = String::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { data } => {
                output.push_str(&String::from_utf8_lossy(&data));
            }
            ChannelMsg::ExtendedData { data, .. } => {
                output.push_str(&String::from_utf8_lossy(&data));
            }
            ChannelMsg::ExitStatus { exit_status } => {
                log::info!("set-inform exit status: {}", exit_status);
            }
            _ => {}
        }
    }

    log::info!("set-inform output: {}", output.trim());

    // The set-inform command typically outputs something like:
    // "Adoption request sent to http://...  Firmware 'BZ.xxx.vX.X.X.xxx.xxx'  AP-ID[...]"
    // Any output without "error" is generally success
    if output.to_lowercase().contains("error") && !output.to_lowercase().contains("inform") {
        return Err(SshError::CommandFailed(format!(
            "set-inform returned an error: {}",
            output.trim()
        )));
    }

    Ok(output.trim().to_string())
}
