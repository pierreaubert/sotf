use super::error::SotfApiClientError;
use super::types::SotfApiErrorResponse;
use super::types::SotfApiResult;
use serde::de::DeserializeOwned;

pub(super) async fn decode_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> SotfApiResult<T> {
    let status = response.status();
    let body = response.bytes().await?;
    if !status.is_success() {
        let message = serde_json::from_slice::<SotfApiErrorResponse>(&body)
            .ok()
            .and_then(|error| error.error)
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(SotfApiClientError::Api {
            status: status.as_u16(),
            message,
        });
    }
    Ok(serde_json::from_slice(&body)?)
}

pub(super) async fn decode_bytes_response(response: reqwest::Response) -> SotfApiResult<Vec<u8>> {
    let status = response.status();
    let body = response.bytes().await?;
    if !status.is_success() {
        let message = serde_json::from_slice::<SotfApiErrorResponse>(&body)
            .ok()
            .and_then(|error| error.error)
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(SotfApiClientError::Api {
            status: status.as_u16(),
            message,
        });
    }
    Ok(body.to_vec())
}
