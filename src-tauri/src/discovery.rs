/// UniFi device discovery via UDP broadcast on port 10001.
///
/// Protocol (see design doc §4.5.3):
/// 1. Send 4-byte packet [0x01, 0x00, 0x00, 0x00] as UDP broadcast to 255.255.255.255:10001
/// 2. Each UniFi device responds with TLV-encoded payload
/// 3. Parse TLV to extract MAC, IP, model, firmware, managed status
use serde::Serialize;
use socket2::{Domain, Protocol, Socket, Type};
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

const DISCOVERY_PORT: u16 = 10001;
const DISCOVERY_PACKET: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const RECV_TIMEOUT_MS: u64 = 5000;
const RECV_BUF_SIZE: usize = 4096;

// TLV field types from the UniFi discovery protocol
const TLV_MAC_ADDRESS: u8 = 0x01;
const TLV_IP_INFO: u8 = 0x02;
const TLV_FIRMWARE: u8 = 0x03;
const TLV_MODEL: u8 = 0x14;
const TLV_PLATFORM: u8 = 0x0B;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub mac: String,
    /// The LAN IP we can actually reach (UDP packet source address)
    pub ip: String,
    /// The IP reported in the TLV payload (may be WAN IP — for display only)
    pub reported_ip: String,
    pub model: String,
    pub firmware: String,
    pub hostname: String,
    pub is_managed: bool,
}

/// Scan the local network for UniFi devices.
/// Returns a list of discovered devices.
pub fn scan_network() -> Result<Vec<DiscoveredDevice>, String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("Failed to create socket: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(RECV_TIMEOUT_MS)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    // Bind to any available port
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    socket
        .bind(&bind_addr.into())
        .map_err(|e| format!("Failed to bind socket: {}", e))?;

    // Send broadcast discovery packet
    let broadcast_addr = SocketAddrV4::new(Ipv4Addr::BROADCAST, DISCOVERY_PORT);
    socket
        .send_to(&DISCOVERY_PACKET, &broadcast_addr.into())
        .map_err(|e| format!("Failed to send discovery packet: {}", e))?;

    log::info!("Sent discovery broadcast on port {}", DISCOVERY_PORT);

    // Collect responses
    let mut devices = Vec::new();
    let mut buf: [MaybeUninit<u8>; RECV_BUF_SIZE] =
        unsafe { MaybeUninit::uninit().assume_init() };
    let deadline = std::time::Instant::now() + Duration::from_millis(RECV_TIMEOUT_MS);

    while std::time::Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                // Safety: recv_from wrote `size` bytes into the buffer
                let received: &[u8] =
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, size) };

                let source_ip = match addr.as_socket() {
                    Some(SocketAddr::V4(v4)) => v4.ip().to_string(),
                    Some(SocketAddr::V6(v6)) => v6.ip().to_string(),
                    None => "unknown".to_string(),
                };
                log::info!("Received {} bytes from {}", size, source_ip);

                if let Some(device) = parse_tlv_response(received, &source_ip) {
                    // Deduplicate by MAC
                    if !devices.iter().any(|d: &DiscoveredDevice| d.mac == device.mac) {
                        devices.push(device);
                    }
                }
            }
            Err(e) => {
                // Timeout or other error — stop listening
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    break;
                }
                log::warn!("recv_from error: {}", e);
                break;
            }
        }
    }

    log::info!("Discovery complete: found {} device(s)", devices.len());
    Ok(devices)
}

/// Parse a TLV-encoded discovery response from a UniFi device.
fn parse_tlv_response(data: &[u8], source_ip: &str) -> Option<DiscoveredDevice> {
    if data.len() < 4 {
        return None;
    }

    let mut mac = String::new();
    let mut reported_ip = source_ip.to_string();
    let mut model = String::new();
    let mut firmware = String::new();
    let mut hostname = String::new();
    let mut is_managed = false;

    // Skip first 4 bytes (response header)
    let mut pos = 4;

    while pos + 4 <= data.len() {
        let field_type = data[pos];
        let field_len = u16::from_be_bytes([data[pos + 1], data[pos + 2]]) as usize;
        pos += 3;

        if pos + field_len > data.len() {
            break;
        }

        let field_data = &data[pos..pos + field_len];

        match field_type {
            TLV_MAC_ADDRESS => {
                if field_len == 6 {
                    mac = field_data
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(":");
                }
            }
            TLV_IP_INFO => {
                if field_len >= 4 {
                    reported_ip = format!(
                        "{}.{}.{}.{}",
                        field_data[0], field_data[1], field_data[2], field_data[3]
                    );
                }
            }
            TLV_FIRMWARE => {
                firmware = String::from_utf8_lossy(field_data).to_string();
            }
            TLV_MODEL => {
                model = String::from_utf8_lossy(field_data).to_string();
            }
            TLV_PLATFORM => {
                hostname = String::from_utf8_lossy(field_data).to_string();
            }
            0x06 => {
                // Managed status / essid — presence suggests managed
                is_managed = true;
            }
            _ => {
                // Unknown field — skip
            }
        }

        pos += field_len;
    }

    if mac.is_empty() {
        return None;
    }

    Some(DiscoveredDevice {
        mac,
        ip: source_ip.to_string(),
        reported_ip,
        model,
        firmware,
        hostname,
        is_managed,
    })
}
