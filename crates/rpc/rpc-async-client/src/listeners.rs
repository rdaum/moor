// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use moor_values::Obj;
use std::net::SocketAddr;

#[derive(Debug, thiserror::Error)]
pub enum ListenersError {
    #[error("Failed to add listener {0:?} at {1}")]
    AddListenerFailed(Obj, SocketAddr),
    #[error("Failed to remove listener at {0}")]
    RemoveListenerFailed(SocketAddr),
    #[error("Failed to get listeners")]
    GetListenersFailed,
}

/// A client for talking to a host-specific backend for managing the set of listeners.
#[derive(Clone)]
pub struct ListenersClient {
    listeners_channel: tokio::sync::mpsc::Sender<ListenersMessage>,
}

pub enum ListenersMessage {
    AddListener(Obj, SocketAddr),
    RemoveListener(SocketAddr),
    GetListeners(tokio::sync::oneshot::Sender<Vec<(Obj, SocketAddr)>>),
}

impl ListenersClient {
    pub fn new(listeners_channel: tokio::sync::mpsc::Sender<ListenersMessage>) -> Self {
        Self { listeners_channel }
    }

    pub async fn add_listener(
        &self,
        handler: &Obj,
        addr: SocketAddr,
    ) -> Result<(), ListenersError> {
        self.listeners_channel
            .send(ListenersMessage::AddListener(handler.clone(), addr))
            .await
            .map_err(|_| ListenersError::AddListenerFailed(handler.clone(), addr))?;
        Ok(())
    }

    pub async fn remove_listener(&self, addr: SocketAddr) -> Result<(), ListenersError> {
        self.listeners_channel
            .send(ListenersMessage::RemoveListener(addr))
            .await
            .map_err(|_| ListenersError::RemoveListenerFailed(addr))?;
        Ok(())
    }

    pub async fn get_listeners(&self) -> Result<Vec<(Obj, SocketAddr)>, ListenersError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.listeners_channel
            .send(ListenersMessage::GetListeners(tx))
            .await
            .map_err(|_| ListenersError::GetListenersFailed)?;
        rx.await.map_err(|_| ListenersError::GetListenersFailed)
    }
}
