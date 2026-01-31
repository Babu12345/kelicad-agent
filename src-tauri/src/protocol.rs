// Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.
// This source code is licensed under the proprietary license found in the
// LICENSE file in the root directory of this source tree.

//! WebSocket protocol types for communication with the web app

use serde::{Deserialize, Serialize};

/// Simulation trace data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub name: String,
    pub data: Vec<f64>,
    pub unit: String,
}

/// Simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResults {
    pub time: Vec<f64>,
    pub traces: Vec<Trace>,
    pub analysis_type: String,
}

/// Agent capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(rename = "ltspiceAvailable")]
    pub ltspice_available: bool,
    #[serde(rename = "ngspiceAvailable")]
    pub ngspice_available: bool,
    #[serde(rename = "supportedAnalyses")]
    pub supported_analyses: Vec<String>,
    #[serde(rename = "maxSimulationTime")]
    pub max_simulation_time: u32,
}

/// Handshake request from web app
#[derive(Debug, Clone, Deserialize)]
pub struct HandshakeRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub origin: String,
    pub version: String,
    pub timestamp: u64,
}

/// Handshake response to web app
#[derive(Debug, Clone, Serialize)]
pub struct HandshakeResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: u64,
    pub success: bool,
    #[serde(rename = "agentVersion")]
    pub agent_version: String,
    #[serde(rename = "ltspicePath", skip_serializing_if = "Option::is_none")]
    pub ltspice_path: Option<String>,
    pub capabilities: AgentCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Simulation request from web app
#[derive(Debug, Clone, Deserialize)]
pub struct SimulationRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub netlist: String,
    #[serde(rename = "waveformQuality")]
    pub waveform_quality: String,
    pub timeout: Option<u64>,
    pub timestamp: u64,
}

/// Simulation response to web app
#[derive(Debug, Clone, Serialize)]
pub struct SimulationResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub timestamp: u64,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<SimulationResults>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "executionTime")]
    pub execution_time: u64,
    pub simulator: String,
}

/// Simulation progress update
#[derive(Debug, Clone, Serialize)]
pub struct SimulationProgress {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub timestamp: u64,
    pub stage: String,
    pub message: String,
}

/// Ping message
#[derive(Debug, Clone, Deserialize)]
pub struct PingMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: u64,
}

/// Pong response
#[derive(Debug, Clone, Serialize)]
pub struct PongResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: u64,
    pub status: String,
}

/// Cancel simulation request
#[derive(Debug, Clone, Deserialize)]
pub struct CancelRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub timestamp: u64,
}

/// Cancel response
#[derive(Debug, Clone, Serialize)]
pub struct CancelResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub timestamp: u64,
    pub success: bool,
}

/// Generic message for type detection
#[derive(Debug, Clone, Deserialize)]
pub struct GenericMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
}

/// Allowed origins for WebSocket connections
pub const ALLOWED_ORIGINS: &[&str] = &[
    "https://kelicad.com",
    "https://www.kelicad.com",
    "http://localhost:3000",
    "http://127.0.0.1:3000",
];

/// Protocol version
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Agent version
pub const AGENT_VERSION: &str = "1.0.0";

/// WebSocket server port
pub const WS_PORT: u16 = 9347;

/// Check if origin is allowed
pub fn is_origin_allowed(origin: &str) -> bool {
    ALLOWED_ORIGINS.contains(&origin)
}

/// Get current timestamp in milliseconds
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_origin_allowed_valid_origins() {
        assert!(is_origin_allowed("https://kelicad.com"));
        assert!(is_origin_allowed("https://www.kelicad.com"));
        assert!(is_origin_allowed("http://localhost:3000"));
        assert!(is_origin_allowed("http://127.0.0.1:3000"));
    }

    #[test]
    fn test_is_origin_allowed_invalid_origins() {
        assert!(!is_origin_allowed("https://malicious.com"));
        assert!(!is_origin_allowed("http://localhost:8080"));
        assert!(!is_origin_allowed("https://kelicad.com.evil.com"));
        assert!(!is_origin_allowed(""));
    }

    #[test]
    fn test_now_ms_returns_reasonable_timestamp() {
        let ts = now_ms();
        // Should be after Jan 1, 2024 (1704067200000 ms)
        assert!(ts > 1704067200000);
        // Should be before Jan 1, 2100 (4102444800000 ms)
        assert!(ts < 4102444800000);
    }

    #[test]
    fn test_handshake_request_deserialization() {
        let json = r#"{
            "id": "test-123",
            "type": "handshake",
            "origin": "https://kelicad.com",
            "version": "1.0.0",
            "timestamp": 1704067200000
        }"#;

        let request: HandshakeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.id, "test-123");
        assert_eq!(request.msg_type, "handshake");
        assert_eq!(request.origin, "https://kelicad.com");
        assert_eq!(request.version, "1.0.0");
        assert_eq!(request.timestamp, 1704067200000);
    }

    #[test]
    fn test_handshake_response_serialization() {
        let response = HandshakeResponse {
            id: "resp-456".to_string(),
            msg_type: "handshake_response".to_string(),
            timestamp: 1704067200000,
            success: true,
            agent_version: "1.0.0".to_string(),
            ltspice_path: Some("/Applications/LTspice.app".to_string()),
            capabilities: AgentCapabilities {
                ltspice_available: true,
                ngspice_available: false,
                supported_analyses: vec!["transient".to_string(), "ac".to_string()],
                max_simulation_time: 120,
            },
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"type\":\"handshake_response\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"agentVersion\":\"1.0.0\""));
        assert!(json.contains("\"ltspicePath\":\"/Applications/LTspice.app\""));
        assert!(json.contains("\"ltspiceAvailable\":true"));
        // Error should be skipped when None
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_handshake_response_with_error() {
        let response = HandshakeResponse {
            id: "resp-789".to_string(),
            msg_type: "handshake_response".to_string(),
            timestamp: 1704067200000,
            success: false,
            agent_version: "1.0.0".to_string(),
            ltspice_path: None,
            capabilities: AgentCapabilities {
                ltspice_available: false,
                ngspice_available: false,
                supported_analyses: vec![],
                max_simulation_time: 120,
            },
            error: Some("Invalid origin".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Invalid origin\""));
        // ltspicePath should be skipped when None
        assert!(!json.contains("\"ltspicePath\""));
    }

    #[test]
    fn test_simulation_request_deserialization() {
        let json = r#"{
            "id": "sim-123",
            "type": "simulate",
            "netlist": "* Test\nV1 in 0 1\n.tran 1m\n.end",
            "waveformQuality": "balanced",
            "timeout": 60000,
            "timestamp": 1704067200000
        }"#;

        let request: SimulationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.id, "sim-123");
        assert_eq!(request.msg_type, "simulate");
        assert!(request.netlist.contains("V1 in 0 1"));
        assert_eq!(request.waveform_quality, "balanced");
        assert_eq!(request.timeout, Some(60000));
    }

    #[test]
    fn test_simulation_request_without_timeout() {
        let json = r#"{
            "id": "sim-456",
            "type": "simulate",
            "netlist": "* Test",
            "waveformQuality": "fast",
            "timestamp": 1704067200000
        }"#;

        let request: SimulationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.timeout, None);
    }

    #[test]
    fn test_simulation_response_serialization() {
        let response = SimulationResponse {
            id: "resp-sim-123".to_string(),
            msg_type: "simulation_result".to_string(),
            request_id: "sim-123".to_string(),
            timestamp: 1704067200000,
            success: true,
            results: Some(SimulationResults {
                time: vec![0.0, 0.001, 0.002],
                traces: vec![
                    Trace {
                        name: "V(out)".to_string(),
                        data: vec![0.0, 0.5, 1.0],
                        unit: "V".to_string(),
                    },
                ],
                analysis_type: "transient".to_string(),
            }),
            error: None,
            execution_time: 1500,
            simulator: "ltspice".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"type\":\"simulation_result\""));
        assert!(json.contains("\"requestId\":\"sim-123\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"executionTime\":1500"));
        assert!(json.contains("\"V(out)\""));
    }

    #[test]
    fn test_simulation_response_with_error() {
        let response = SimulationResponse {
            id: "resp-sim-456".to_string(),
            msg_type: "simulation_result".to_string(),
            request_id: "sim-456".to_string(),
            timestamp: 1704067200000,
            success: false,
            results: None,
            error: Some("LTspice not found".to_string()),
            execution_time: 50,
            simulator: "ltspice".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"LTspice not found\""));
        assert!(!json.contains("\"results\""));
    }

    #[test]
    fn test_generic_message_deserialization() {
        let json = r#"{"id": "test", "type": "ping"}"#;
        let msg: GenericMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "ping");
    }

    #[test]
    fn test_ping_pong_messages() {
        let ping_json = r#"{
            "id": "ping-123",
            "type": "ping",
            "timestamp": 1704067200000
        }"#;

        let ping: PingMessage = serde_json::from_str(ping_json).unwrap();
        assert_eq!(ping.msg_type, "ping");

        let pong = PongResponse {
            id: "pong-123".to_string(),
            msg_type: "pong".to_string(),
            timestamp: 1704067200001,
            status: "ready".to_string(),
        };

        let pong_json = serde_json::to_string(&pong).unwrap();
        assert!(pong_json.contains("\"type\":\"pong\""));
        assert!(pong_json.contains("\"status\":\"ready\""));
    }

    #[test]
    fn test_simulation_progress_serialization() {
        let progress = SimulationProgress {
            id: "prog-123".to_string(),
            msg_type: "simulation_progress".to_string(),
            request_id: "sim-123".to_string(),
            timestamp: 1704067200000,
            stage: "running".to_string(),
            message: "Executing simulation...".to_string(),
        };

        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"type\":\"simulation_progress\""));
        assert!(json.contains("\"requestId\":\"sim-123\""));
        assert!(json.contains("\"stage\":\"running\""));
    }

    #[test]
    fn test_constants() {
        assert_eq!(PROTOCOL_VERSION, "1.0.0");
        assert_eq!(AGENT_VERSION, "1.0.0");
        assert_eq!(WS_PORT, 9347);
        assert_eq!(ALLOWED_ORIGINS.len(), 4);
    }
}
