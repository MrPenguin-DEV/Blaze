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
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCSdpType,
    RTCConfigurationBuilder, RTCIceCandidateInit, RTCIceServer, RTCIceTransportPolicy,
    RTCPeerConnectionIceEvent, RTCSessionDescription,
};

/// Signaling payloads exchanged through an authenticated out-of-band service.
/// ICE candidates are sent separately so the connection can use trickle ICE.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Only accept TURN relay candidates. This hides host/reflexive IP
    /// candidates from the remote peer but requires a working TURN server.
    pub relay_only: bool,
}

/// Builds one ICE server from explicit user configuration. A URL may be STUN,
/// STUNS, TURN, or TURNS. TURN endpoints require non-empty credentials.
pub fn build_ice_server(
    urls: Vec<String>,
    username: String,
    credential: String,
) -> Result<RTCIceServer, String> {
    let urls: Vec<String> = urls.into_iter().map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty()).collect();
    if urls.is_empty() {
        return Err("At least one STUN/TURN URL is required.".to_owned());
    }

    let mut needs_turn_credentials = false;
    for url in &urls {
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("turn:") || lower.starts_with("turns:") {
            needs_turn_credentials = true;
        } else if !(lower.starts_with("stun:") || lower.starts_with("stuns:")) {
            return Err(format!("Unsupported ICE URL scheme: {url}"));
        }
    }

    if needs_turn_credentials && (username.trim().is_empty() || credential.is_empty()) {
        return Err("TURN URLs require both a username and a credential.".to_owned());
    }

    Ok(RTCIceServer {
        urls,
        username,
        credential,
        ..Default::default()
    })
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
    inbound: mpsc::UnboundedSender<String>,
}

async fn forward_data_channel(channel: Arc<dyn DataChannel>, inbound: mpsc::UnboundedSender<String>) {
    while let Some(event) = channel.poll().await {
        if let DataChannelEvent::OnMessage(message) = event {
            if let Ok(text) = String::from_utf8(message.data.to_vec()) {
                let _ = inbound.send(text);
            }
        }
    }
}

#[async_trait]
impl PeerConnectionEventHandler for SignalHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        if let Ok(candidate) = event.candidate.to_json() {
            let _ = self.outbound.send(SignalingMessage::Candidate(candidate));
        }
    }

    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        tokio::spawn(forward_data_channel(channel, self.inbound.clone()));
    }
}

/// An established local WebRTC peer connection plus the reliable text channel
/// used by Blaze Messenger. The data channel is not writable until its DCEP
/// handshake completes.
pub struct PeerSession {
    peer: Arc<dyn PeerConnection>,
    data_channel: Arc<dyn DataChannel>,
    outbound_signaling: mpsc::UnboundedReceiver<SignalingMessage>,
    inbound_messages: mpsc::UnboundedReceiver<String>,
}

impl PeerSession {
    /// Creates a peer connection and an ordered, reliable data channel.
    /// No public STUN/TURN server is silently selected.
    pub async fn new(config: PeerConfig) -> Result<Self, CommsError> {
        let (signal_tx, signal_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let mut configuration = RTCConfigurationBuilder::default()
            .with_ice_servers(config.ice_servers);
        if config.relay_only {
            configuration = configuration.with_ice_transport_policy(RTCIceTransportPolicy::Relay);
        }
        let configuration = configuration.build();

        let mut builder = PeerConnectionBuilder::new()
            .with_configuration(configuration)
            .with_handler(Arc::new(SignalHandler {
                outbound: signal_tx,
                inbound: message_tx.clone(),
            }));

        if !config.udp_bind_addresses.is_empty() {
            builder = builder.with_udp_addrs(config.udp_bind_addresses);
        }

        let peer: Arc<dyn PeerConnection> = Arc::new(builder.build().await?);
        let data_channel = peer.create_data_channel("blaze-messenger", None).await?;
        tokio::spawn(forward_data_channel(data_channel.clone(), message_tx));

        Ok(Self {
            peer,
            data_channel,
            outbound_signaling: signal_rx,
            inbound_messages: message_rx,
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

    /// Drains all currently queued ICE candidates without blocking the UI.
    pub fn drain_outbound_signaling(&mut self) -> Vec<SignalingMessage> {
        let mut messages = Vec::new();
        while let Ok(message) = self.outbound_signaling.try_recv() {
            messages.push(message);
        }
        messages
    }

    /// Drains currently queued UTF-8 messages received over the data channel.
    pub fn drain_incoming_text(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        while let Ok(message) = self.inbound_messages.try_recv() {
            messages.push(message);
        }
        messages
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

/// Normalizes a SHA-256 DTLS certificate fingerprint to 64 lowercase hex
/// characters, accepting the colon-separated form used by SDP.
pub fn normalize_sha256_fingerprint(value: &str) -> Option<String> {
    let normalized: String = value
        .chars()
        .filter(|character| *character != ':' && !character.is_ascii_whitespace())
        .flat_map(char::to_lowercase)
        .collect();

    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(normalized)
}

/// Extracts the SHA-256 DTLS fingerprint from a session description.
/// If multiple media sections advertise fingerprints, they must agree.
pub fn sha256_dtls_fingerprint(sdp: &str) -> Option<String> {
    let mut fingerprints = Vec::new();

    for line in sdp.lines() {
        let Some(attribute) = line.trim().strip_prefix("a=fingerprint:") else {
            continue;
        };
        let mut parts = attribute.split_whitespace();
        let algorithm = parts.next()?;
        let value = parts.next()?;

        if !algorithm.eq_ignore_ascii_case("sha-256") {
            continue;
        }
        let fingerprint = normalize_sha256_fingerprint(value)?;
        if !fingerprints.contains(&fingerprint) {
            fingerprints.push(fingerprint);
        }
    }

    if fingerprints.len() == 1 {
        fingerprints.pop()
    } else {
        None
    }
}

/// Verifies the SHA-256 fingerprint in an SDP description against a value
/// confirmed independently with the intended contact.
pub fn verify_sha256_dtls_fingerprint(sdp: &str, trusted: &str) -> bool {
    match (
        sha256_dtls_fingerprint(sdp),
        normalize_sha256_fingerprint(trusted),
    ) {
        (Some(observed), Some(expected)) => observed == expected,
        _ => false,
    }
}

/// A transport-security warning suitable for the Messenger UI.
pub const IDENTITY_WARNING: &str =
    "WebRTC encrypts the transport, but identity is not automatic. Compare the SHA-256 DTLS fingerprint with your contact over a separate trusted channel before sending sensitive messages.";

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
        match decoded {
            SignalingMessage::Description(decoded_description) => {
                assert_eq!(decoded_description.sdp, description.sdp);
                assert_eq!(decoded_description.sdp_type, description.sdp_type);
            }
            SignalingMessage::Candidate(_) => panic!("expected a description payload"),
        }
    }

    #[test]
    fn normalizes_and_verifies_sha256_dtls_fingerprint() {
        let fingerprint = "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF";
        let sdp = format!(
            "v=0\r\na=fingerprint:sha-256 {fingerprint}\r\n"
        );
        let canonical = normalize_sha256_fingerprint(fingerprint).unwrap();
        assert_eq!(canonical.len(), 64);
        assert!(verify_sha256_dtls_fingerprint(&sdp, &canonical));
        assert!(!verify_sha256_dtls_fingerprint(&sdp, "00"));
    }

    #[test]
    fn refuses_missing_or_conflicting_fingerprints() {
        assert!(sha256_dtls_fingerprint("v=0\r\ns=-\r\n").is_none());
        let first = "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF";
        let second = "11:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF";
        let sdp = format!("a=fingerprint:sha-256 {first}\na=fingerprint:sha-256 {second}\n");
        assert!(sha256_dtls_fingerprint(&sdp).is_none());
    }

    #[test]
    fn empty_peer_config_does_not_add_public_ice_services() {
        let config = PeerConfig::default();
        assert!(config.ice_servers.is_empty());
        assert!(config.udp_bind_addresses.is_empty());
        assert!(!config.relay_only);
    }

    #[test]
    fn accepts_explicit_stun_server_without_credentials() {
        let server = build_ice_server(
            vec!["stun:stun.example.org:3478".into()],
            String::new(),
            String::new(),
        )
        .unwrap();
        assert_eq!(server.urls, vec!["stun:stun.example.org:3478"]);
    }

    #[test]
    fn requires_credentials_for_turn_servers() {
        assert!(build_ice_server(
            vec!["turn:turn.example.org:3478".into()],
            String::new(),
            String::new(),
        ).is_err());
        assert!(build_ice_server(
            vec!["turns:turn.example.org:5349".into()],
            "blaze-user".into(),
            "token".into(),
        ).is_ok());
    }

    #[test]
    fn rejects_non_ice_url_schemes() {
        assert!(build_ice_server(
            vec!["https://example.org".into()],
            String::new(),
            String::new(),
        ).is_err());
    }
}
