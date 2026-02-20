/// HTTP client for the /api/setup-code endpoint.
///
/// The companion app calls this to validate a setup code and retrieve
/// the inform URL and site metadata (see design doc §4.6.2).
use serde::{Deserialize, Serialize};

// Default to the production wizard URL — can be overridden for dev
const DEFAULT_API_BASE: &str = "https://ubiquitywizard.onrender.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupCodeResponse {
    pub inform_url: String,
    pub site_id: String,
    pub site_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCodeError {
    pub error: String,
    #[serde(default)]
    pub expired: bool,
}

#[derive(Debug)]
pub enum ApiError {
    InvalidCode(String),
    ExpiredCode(String),
    NetworkError(String),
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::InvalidCode(msg) => write!(f, "{}", msg),
            ApiError::ExpiredCode(msg) => write!(f, "{}", msg),
            ApiError::NetworkError(msg) => write!(f, "{}", msg),
            ApiError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

/// Validate a setup code against the VivaSpot API.
/// Returns the inform URL and site metadata on success.
pub async fn validate_setup_code(code: &str) -> Result<SetupCodeResponse, ApiError> {
    let api_base = std::env::var("VIVASPOT_API_URL").unwrap_or_else(|_| DEFAULT_API_BASE.to_string());
    let url = format!("{}/api/setup-code?code={}", api_base, code);

    log::info!("Validating setup code: {}", code);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                ApiError::NetworkError(
                    "Can't connect to VivaSpot. Check your internet connection.".to_string(),
                )
            } else {
                ApiError::Other(format!("Request failed: {}", e))
            }
        })?;

    if response.status().is_success() {
        let data: SetupCodeResponse = response
            .json()
            .await
            .map_err(|e| ApiError::Other(format!("Failed to parse response: {}", e)))?;

        log::info!("Setup code valid — site: {}, inform URL: {}", data.site_name, data.inform_url);
        Ok(data)
    } else if response.status().as_u16() == 404 {
        let err: SetupCodeError = response
            .json()
            .await
            .map_err(|e| ApiError::Other(format!("Failed to parse error: {}", e)))?;

        if err.expired {
            Err(ApiError::ExpiredCode(err.error))
        } else {
            Err(ApiError::InvalidCode(err.error))
        }
    } else {
        Err(ApiError::Other(format!(
            "Unexpected response: {}",
            response.status()
        )))
    }
}
