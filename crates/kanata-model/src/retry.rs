//! Retry logic for LLM API calls with rate-limit and server-error handling.

use std::time::Duration;

use reqwest::StatusCode;
use tracing::{info, warn};

use kanata_types::KanataError;

/// Maximum number of retries for retriable errors.
const MAX_RETRIES: u32 = 3;

/// Default connection/request timeout.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Execute an async HTTP-producing closure with retry logic.
///
/// Retries on 429 (rate limit) and 5xx (server error) responses.
///
/// # Errors
///
/// Returns `KanataError::ModelError` if retries are exhausted or a non-retriable
/// HTTP error is encountered. Propagates any error from the closure itself.
pub async fn with_retry<F, Fut>(mut f: F) -> Result<reqwest::Response, KanataError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, KanataError>>,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let resp = f().await?;
        let status = resp.status();

        if status.is_success() {
            return Ok(resp);
        }

        if attempt >= MAX_RETRIES {
            let body = resp.text().await.unwrap_or_default();
            return Err(KanataError::ModelError {
                status: status.as_u16(),
                message: body,
            });
        }

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(5);
            info!(attempt, wait_secs = wait, "rate limited, waiting");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }

        if status.is_server_error() {
            let wait = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s, 4s
            warn!(attempt, status = %status, ?wait, "server error, retrying");
            tokio::time::sleep(wait).await;
            continue;
        }

        // Non-retriable error (4xx except 429)
        let body = resp.text().await.unwrap_or_default();
        return Err(KanataError::ModelError {
            status: status.as_u16(),
            message: body,
        });
    }
}
