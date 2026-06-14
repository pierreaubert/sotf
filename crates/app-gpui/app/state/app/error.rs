use super::remote_refresh_requests::RemoteRefreshRequests;

#[derive(Debug)]
pub struct RemoteCacheRefreshError {
    pub requests: RemoteRefreshRequests,
    pub message: String,
}
