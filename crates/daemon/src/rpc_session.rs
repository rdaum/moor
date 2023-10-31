use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::trace;
use uuid::Uuid;

use moor_kernel::tasks::sessions::{Session, SessionError};
use moor_values::model::NarrativeEvent;
use moor_values::var::objid::Objid;

use crate::rpc_server::RpcServer;

/// A "session" that runs over the RPC system.
pub struct RpcSession {
    client_id: Uuid,
    rpc_server: Arc<RpcServer>,
    player: Objid,
    // TODO: manage this buffer better -- e.g. if it grows too big, for long-running tasks, etc. it
    //  should be mmap'd to disk or something.
    // TODO: We could also use Boxcar or other append-only lockless container for this, since we only
    //  ever append.
    session_buffer: Mutex<Vec<(Objid, NarrativeEvent)>>,
}

impl RpcSession {
    pub fn new(client_id: Uuid, rpc_server: Arc<RpcServer>, player: Objid) -> Self {
        Self {
            client_id,
            rpc_server,
            player,
            session_buffer: Default::default(),
        }
    }
}

#[async_trait]
impl Session for RpcSession {
    async fn commit(&self) -> Result<(), SessionError> {
        trace!(player = ?self.player, client_id = ?self.client_id, "Committing session");
        let events: Vec<_> = {
            let mut session_buffer = self.session_buffer.lock().await;
            session_buffer.drain(..).collect()
        };

        self.rpc_server
            .publish_narrative_events(&events[..])
            .await
            .map_err(|e| SessionError::CommitError(e.to_string()))?;

        Ok(())
    }

    async fn rollback(&self) -> Result<(), SessionError> {
        let mut session_buffer = self.session_buffer.lock().await;
        session_buffer.clear();
        Ok(())
    }

    async fn fork(self: Arc<Self>) -> Result<Arc<dyn Session>, SessionError> {
        // We ask the rpc server to create a new session, otherwise we'd need to have a copy of all
        // the info to create a Publish. The rpc server has that, though.
        let new_session = self
            .rpc_server
            .clone()
            .new_session(self.client_id, self.player)
            .await?;
        Ok(new_session)
    }

    async fn request_input(
        &self,
        player: Objid,
        input_request_id: Uuid,
    ) -> Result<(), SessionError> {
        self.rpc_server
            .clone()
            .request_client_input(self.client_id, player, input_request_id)
            .await?;
        Ok(())
    }

    async fn send_event(&self, player: Objid, event: NarrativeEvent) -> Result<(), SessionError> {
        self.session_buffer.lock().await.push((player, event));
        Ok(())
    }

    async fn send_system_msg(&self, player: Objid, msg: &str) -> Result<(), SessionError> {
        self.rpc_server
            .send_system_message(self.client_id, player, msg.to_string())
            .await?;
        Ok(())
    }

    async fn shutdown(&self, _msg: Option<String>) -> Result<(), SessionError> {
        todo!()
    }

    async fn connection_name(&self, player: Objid) -> Result<String, SessionError> {
        self.rpc_server.connection_name_for(player).await
    }

    async fn disconnect(&self, player: Objid) -> Result<(), SessionError> {
        self.rpc_server.disconnect(player).await
    }

    async fn connected_players(&self) -> Result<Vec<Objid>, SessionError> {
        self.rpc_server.connected_players().await
    }

    async fn connected_seconds(&self, player: Objid) -> Result<f64, SessionError> {
        self.rpc_server.connected_seconds_for(player).await
    }

    async fn idle_seconds(&self, player: Objid) -> Result<f64, SessionError> {
        self.rpc_server.idle_seconds_for(player).await
    }
}
