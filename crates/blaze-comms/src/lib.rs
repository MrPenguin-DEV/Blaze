//! WebRTC call and messenger transport foundation.
//!
//! This crate creates real peer connections and a reliable data channel. It
//! does not include a signaling server, contact identity, certificate
//! fingerprint verification, device capture, or TURN credentials. Applications
//! must authenticate signaling and verify peer identity before treating a
//! connection as a trusted private conversation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use webrtc::data_channel::DataChannel;
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, RTCIceCandidateInit, RTCIceServer, RTCPeerConnectionIceEvent,
    RTCSessionDescription,
};
use webrtc::peer_connection::sdp::RTCSdpType;

/// Signaling payloads exchanged through an authenticated out-of-band service.
/// ICE candidates are sent separately so the connection can use trickle ICE.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "payload")]
pub enum SignalingMessage {
    Description(RTCSessionDescription),
    Candidate(RTCIceCandidateInit),
}

/// Explicit connection setup. Empty ICE server list means host candidates
/// only; provide a TURN service when connections must work across NATs.
#[derive(Clone, Debug, Default)]
pub struct PeerConfig {
    pub ice_servers: Vec<RTCIceServer>,
    pub udp_bind_addresses: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CommsError {
    #[error("WebRTC operation failed: {0}")]
    WebRtc(#[from] webrtc::error::Error),
    #[error("peer connection did not expose a local session description")]
    MissingLocalDescription,
}

struct SignalHandler {
    outbound: mpsc::UnboundedSender<SignalingMessage>,
}

#[async_trait]
impl PeerConnectionEventHandler for SignalHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        if let Ok(candidate) = event.candidate.to_json() {
            let _ = self.outbound.send(SignalingMessage::Candidate(candidate));
        }
    }
}

/// An established local WebRTC peer connection plus the reliable text channel
/// used by Blaze Messenger. The data channel is not writable until its DCEP
/// handshake completes.
pub struct PeerSession {
    peer: Arc<dyn PeerConnection>,
    data_channel: Arc<dyn DataChannel>,
    outbound_signaling: mpsc::UnboundedReceiver<SignalingMessage>,
}

impl PeerSession {
    /// Creates a peer connection and an ordered, reliable data channel.
    /// No public STUN/TURN server is silently selected.
    pub async fn new(config: PeerConfig) -> Result<Self, CommsError> {
        let (signal_tx, signal_rx) = mpsc::unbounded_channel();
        let configuration = RTCConfigurationBuilder::new()
            .with_ice_servers(config.ice_servers)
            .build();

        let mut builder = PeerConnectionBuilder::new()
            .with_configuration(configuration)
            .with_handler(Arc::new(SignalHandler { outbound: signal_tx }));

        if !config.udp_bind_addresses.is_empty() {
            builder = builder.with_udp_addrs(config.udp_bind_addresses);
        }

        let peer = builder.build().await?;
        let data_channel = peer.create_data_channel("blaze-messenger", None).await?;

        Ok(Self {
            peer,
            data_channel,
            outbound_signaling: signal_rx,
        })
    }

    /// Generates an SDP offer, sets it locally, and returns the local
    /// description to be sent over an authenticated signaling service.
    pub async fn create_offer(&self) -> Result<RTCSessionDescription, CommsError> {
        let offer = self.peer.create_offer(None).await?;
        self.peer.set_local_description(offer).await?;
        self.peer
            .local_description()
            .await
            .ok_or(CommsError::MissingLocalDescription)
    }

    /// Applies a remote offer and generates a local answer. For a remote
    /// answer, this only applies the answer and returns None.
    pub async fn handle_signaling(
        &self,
        message: SignalingMessage,
    ) -> Result<Option<SignalingMessage>, CommsError> {
        match message {
            SignalingMessage::Candidate(candidate) => {
                self.peer.add_ice_candidate(candidate).await?;
                Ok(None)
            }
            SignalingMessage::Description(description) => {
                let is_offer = description.sdp_type == RTCSdpType::Offer;
                self.peer.set_remote_description(description).await?;
                if !is_offer {
                    return Ok(None);
                }

                let answer = self.peer.create_answer(None).await?;
                self.peer.set_local_description(answer).await?;
                let local = self
                    .peer
                    .local_description()
                    .await
                    .ok_or(CommsError::MissingLocalDescription)?;
                Ok(Some(SignalingMessage::Description(local)))
            }
        }
    }

    /// Receives the next local ICE candidate generated by the WebRTC stack.
    /// Forward the result to the authenticated signaling channel.
    pub async fn next_outbound_signaling(&mut self) -> Option<SignalingMessage> {
        self.outbound_signaling.recv().await
    }

    /// Waits for the data channel to become writable.
    pub async fn wait_until_writable(&self) -> Result<(), CommsError> {
        self.data_channel.writable().await?;
        Ok(())
    }

    /// Sends a reliable UTF-8 message over the WebRTC data channel.
    pub async fn send_text(&self, message: &str) -> Result<(), CommsError> {
        self.data_channel.send_text(message).await?;
        Ok(())
    }

    /// Closes both the data channel and the underlying peer connection.
    pub async fn close(&self) -> Result<(), CommsError> {
        self.data_channel.close().await?;
        self.peer.close().await?;
        Ok(())
    }
}

/// A transport-security warning suitable for the Messenger UI.
pub const IDENTITY_WARNING: &str =
    "WebRTC encrypts the transport, but Blaze has not yet authenticated peer identity or pinned DTLS certificate fingerprints. Do not treat the session as verified end-to-end encrypted messaging.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signaling_messages_round_trip_as_json() {
        let description = RTCSessionDescription::offer(
            "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n".to_owned()
        ).unwrap();
        let message = SignalingMessage::Description(description);
        let serialized = serde_json::to_string(&message).unwrap();
        let decoded: SignalingMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(decoded, message);
    }

    #[test]
    fn empty_peer_config_does_not_add_public_ice_services() {
        let config = PeerConfig::default();
        assert!(config.ice_servers.is_empty());
        assert!(config.udp_bind_addresses.is_empty());
    }
}
