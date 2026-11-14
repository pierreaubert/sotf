use super::sotf_remote_server::SotfRemoteServer;
use super::types::SotfRemoteConnectionInfo;
use super::types::SotfRemoteSnapshot;
use super::types::SotfRemoteTransportCommand;
use crate::sotf_api_client::{
    SotfApiAlbumList, SotfApiClient, SotfApiCommandResponse, SotfApiQueue,
    SotfApiQueueEditResponse, SotfApiResult, SotfApiState,
};

#[derive(Clone, Debug)]
pub struct SotfRemoteConnection {
    pub server: SotfRemoteServer,
    pub client: SotfApiClient,
}

impl SotfRemoteConnection {
    /// Validate.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn validate(&self) -> SotfApiResult<SotfRemoteConnectionInfo> {
        let health = self.client.health().await?;
        let discovery = self.client.discovery().await?;
        let capabilities = self.client.capabilities().await?;
        let state = self.client.state().await?;
        Ok(SotfRemoteConnectionInfo {
            health,
            discovery,
            capabilities,
            state,
        })
    }

    /// Refresh.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn refresh(&self) -> SotfApiResult<SotfRemoteSnapshot> {
        let state = self.client.state().await?;
        let queue = self.client.queue().await?;
        let library = self
            .client
            .library_albums_page(0, 50, None, Some("artist_title"))
            .await?;
        Ok(SotfRemoteSnapshot {
            state,
            queue,
            library,
        })
    }

    /// Refresh state.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn refresh_state(&self) -> SotfApiResult<SotfApiState> {
        self.client.state().await
    }

    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn refresh_queue(&self) -> SotfApiResult<SotfApiQueue> {
        self.client.queue().await
    }

    /// Refresh album page.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn refresh_album_page(
        &self,
        offset: usize,
        limit: usize,
        query: Option<&str>,
        sort: Option<&str>,
    ) -> SotfApiResult<SotfApiAlbumList> {
        self.client
            .library_albums_page(offset, limit, query, sort)
            .await
    }

    /// Album artwork.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn album_artwork(&self, album_id: &str) -> SotfApiResult<Vec<u8>> {
        self.client.album_artwork(album_id).await
    }

    /// Transport.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn transport(
        &self,
        command: SotfRemoteTransportCommand,
    ) -> SotfApiResult<SotfApiCommandResponse> {
        match command {
            SotfRemoteTransportCommand::Play => self.client.play().await,
            SotfRemoteTransportCommand::Pause => self.client.pause().await,
            SotfRemoteTransportCommand::Resume => self.client.resume().await,
            SotfRemoteTransportCommand::Stop => self.client.stop().await,
            SotfRemoteTransportCommand::Next => self.client.next().await,
            SotfRemoteTransportCommand::Previous => self.client.previous().await,
        }
    }

    /// Seek.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn seek(&self, position_secs: f64) -> SotfApiResult<SotfApiCommandResponse> {
        self.client.seek(position_secs).await
    }

    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn set_volume(&self, volume: u8) -> SotfApiResult<SotfApiCommandResponse> {
        self.client.set_volume(volume).await
    }

    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn add_album_to_queue(
        &self,
        album_id: impl Into<String>,
        play_now: bool,
    ) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_add_album(album_id, play_now).await
    }

    /// Clear queue.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn clear_queue(&self) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_clear().await
    }

    /// Delete queue item.
    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn delete_queue_item(&self, index: usize) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_delete(index).await
    }

    ///
    /// # Errors
    /// Returns an error if the remote request fails, is rejected, or returns an invalid response.
    pub async fn jump_to_queue_item(
        &self,
        index: usize,
    ) -> SotfApiResult<SotfApiQueueEditResponse> {
        self.client.queue_jump(index).await
    }
}
