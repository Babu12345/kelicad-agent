// Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.
// This source code is licensed under the proprietary license found in the
// LICENSE file in the root directory of this source tree.

//! WebSocket server for handling connections from the web app

use std::sync::Arc;
use std::sync::atomic::Ordering;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::protocol::*;
use crate::simulator;
use crate::AppState;

/// Start the WebSocket server
pub async fn start_server(state: Arc<AppState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("127.0.0.1:{}", WS_PORT);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("WebSocket server listening on {}", addr);

    while let Ok((stream, peer_addr)) = listener.accept().await {
        log::info!("New connection from: {}", peer_addr);
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state).await {
                log::error!("Connection error: {}", e);
            }
        });
    }

    Ok(())
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    state: Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    // Increment connection count
    {
        let mut count = state.ws_connections.write().await;
        *count += 1;
    }

    // Track if handshake was successful
    let mut handshake_complete = false;

    // Channel for simulation results
    let (sim_tx, mut sim_rx) = mpsc::channel::<String>(1);

    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        log::error!("WebSocket read error: {}", e);
                        break;
                    }
                    None => break,
                };

                if let Message::Text(text) = msg {
                    // Parse the message type first
                    let generic: GenericMessage = match serde_json::from_str(&text) {
                        Ok(m) => m,
                        Err(e) => {
                            log::error!("Failed to parse message: {}", e);
                            continue;
                        }
                    };

                    let response = match generic.msg_type.as_str() {
                        "handshake" => {
                            let request: HandshakeRequest = serde_json::from_str(&text)?;
                            let response = handle_handshake(&request, &state).await;
                            handshake_complete = response.success;
                            Some(serde_json::to_string(&response)?)
                        }
                        "simulate" => {
                            if !handshake_complete {
                                log::warn!("Simulation request before handshake");
                                continue;
                            }
                            let request: SimulationRequest = serde_json::from_str(&text)?;

                            // Send progress update
                            let progress = SimulationProgress {
                                id: uuid::Uuid::new_v4().to_string(),
                                msg_type: "simulation_progress".to_string(),
                                request_id: request.id.clone(),
                                timestamp: now_ms(),
                                stage: "preparing".to_string(),
                                message: "Preparing simulation...".to_string(),
                            };
                            write.send(Message::Text(serde_json::to_string(&progress)?)).await?;

                            // Spawn simulation in a separate task so we can process cancel messages
                            let state_clone = state.clone();
                            let sim_tx_clone = sim_tx.clone();
                            tokio::spawn(async move {
                                let response = handle_simulate(&request, &state_clone).await;
                                let _ = sim_tx_clone.send(serde_json::to_string(&response).unwrap_or_default()).await;
                            });
                            None // Don't send response immediately, it will come via sim_rx
                        }
                        "ping" => {
                            let _request: PingMessage = serde_json::from_str(&text)?;
                            let is_sim = *state.is_simulating.read().await;
                            let response = PongResponse {
                                id: uuid::Uuid::new_v4().to_string(),
                                msg_type: "pong".to_string(),
                                timestamp: now_ms(),
                                status: if is_sim { "busy" } else { "ready" }.to_string(),
                            };
                            Some(serde_json::to_string(&response)?)
                        }
                        "cancel" => {
                            let request: CancelRequest = serde_json::from_str(&text)?;
                            let response = handle_cancel(&request, &state).await;
                            Some(serde_json::to_string(&response)?)
                        }
                        _ => {
                            log::warn!("Unknown message type: {}", generic.msg_type);
                            continue;
                        }
                    };

                    if let Some(response) = response {
                        if let Err(e) = write.send(Message::Text(response)).await {
                            log::error!("Failed to send response: {}", e);
                            break;
                        }
                    }
                }
            }

            // Handle simulation results from spawned tasks
            Some(response) = sim_rx.recv() => {
                if let Err(e) = write.send(Message::Text(response)).await {
                    log::error!("Failed to send response: {}", e);
                    break;
                }
            }
        }
    }

    // Decrement connection count
    {
        let mut count = state.ws_connections.write().await;
        *count = count.saturating_sub(1);
    }

    log::info!("Connection closed");
    Ok(())
}

/// Handle handshake request
async fn handle_handshake(request: &HandshakeRequest, state: &AppState) -> HandshakeResponse {
    // Validate origin
    if !is_origin_allowed(&request.origin) {
        log::warn!("Rejected connection from origin: {}", request.origin);
        return HandshakeResponse {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type: "handshake_response".to_string(),
            timestamp: now_ms(),
            success: false,
            agent_version: AGENT_VERSION.to_string(),
            ltspice_path: None,
            capabilities: AgentCapabilities {
                ltspice_available: false,
                ngspice_available: false,
                supported_analyses: vec![],
                max_simulation_time: 120,
            },
            error: Some("Invalid origin".to_string()),
        };
    }

    let ltspice_path = state.ltspice_path.read().await.clone();
    let ltspice_available = ltspice_path.is_some();

    log::info!("Handshake successful from: {}", request.origin);

    HandshakeResponse {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "handshake_response".to_string(),
        timestamp: now_ms(),
        success: true,
        agent_version: AGENT_VERSION.to_string(),
        ltspice_path,
        capabilities: AgentCapabilities {
            ltspice_available,
            ngspice_available: false,
            supported_analyses: vec![
                "transient".to_string(),
                "ac".to_string(),
                "dc".to_string(),
            ],
            max_simulation_time: 120,
        },
        error: None,
    }
}

/// Handle simulation request
async fn handle_simulate(request: &SimulationRequest, state: &AppState) -> SimulationResponse {
    let start_time = std::time::Instant::now();

    // Check if already simulating
    {
        let is_sim = *state.is_simulating.read().await;
        if is_sim {
            return SimulationResponse {
                id: uuid::Uuid::new_v4().to_string(),
                msg_type: "simulation_result".to_string(),
                request_id: request.id.clone(),
                timestamp: now_ms(),
                success: false,
                results: None,
                error: Some("Another simulation is already running".to_string()),
                execution_time: 0,
                simulator: "ltspice".to_string(),
            };
        }
    }

    // Check if LTspice is available
    let ltspice_path = state.ltspice_path.read().await.clone();
    let ltspice_path = match ltspice_path {
        Some(p) => p,
        None => {
            return SimulationResponse {
                id: uuid::Uuid::new_v4().to_string(),
                msg_type: "simulation_result".to_string(),
                request_id: request.id.clone(),
                timestamp: now_ms(),
                success: false,
                results: None,
                error: Some("LTspice not found on this system".to_string()),
                execution_time: 0,
                simulator: "ltspice".to_string(),
            };
        }
    };

    // Mark as simulating and set current simulation ID
    {
        let mut is_sim = state.is_simulating.write().await;
        *is_sim = true;
        let mut current_id = state.current_simulation_id.write().await;
        *current_id = Some(request.id.clone());
        state.cancel_requested.store(false, Ordering::SeqCst);
        state.current_process_id.store(0, Ordering::SeqCst);
    }

    // Run simulation with the process ID holder
    let result = simulator::run_ltspice_simulation(
        &ltspice_path,
        &request.netlist,
        &request.waveform_quality,
        Some(state.current_process_id.clone()),
    )
    .await;

    // Check if cancelled
    let was_cancelled = state.cancel_requested.load(Ordering::SeqCst);

    // Mark as not simulating and clear current simulation ID
    {
        let mut is_sim = state.is_simulating.write().await;
        *is_sim = false;
        let mut current_id = state.current_simulation_id.write().await;
        *current_id = None;
        state.cancel_requested.store(false, Ordering::SeqCst);
        state.current_process_id.store(0, Ordering::SeqCst);
    }

    // If cancelled, return cancelled error
    if was_cancelled {
        return SimulationResponse {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type: "simulation_result".to_string(),
            request_id: request.id.clone(),
            timestamp: now_ms(),
            success: false,
            results: None,
            error: Some("Simulation cancelled".to_string()),
            execution_time: start_time.elapsed().as_millis() as u64,
            simulator: "ltspice".to_string(),
        };
    }

    let execution_time = start_time.elapsed().as_millis() as u64;

    match result {
        Ok(results) => {
            log::info!(
                "Simulation completed: {} traces, {} points",
                results.traces.len(),
                results.time.len()
            );

            // Update simulation stats
            {
                let mut count = state.simulation_count.write().await;
                *count += 1;
                let mut last_time = state.last_simulation_time.write().await;
                *last_time = Some(now_ms());
            }

            SimulationResponse {
                id: uuid::Uuid::new_v4().to_string(),
                msg_type: "simulation_result".to_string(),
                request_id: request.id.clone(),
                timestamp: now_ms(),
                success: true,
                results: Some(results),
                error: None,
                execution_time,
                simulator: "ltspice".to_string(),
            }
        }
        Err(e) => {
            log::error!("Simulation failed: {}", e);
            SimulationResponse {
                id: uuid::Uuid::new_v4().to_string(),
                msg_type: "simulation_result".to_string(),
                request_id: request.id.clone(),
                timestamp: now_ms(),
                success: false,
                results: None,
                error: Some(e.to_string()),
                execution_time,
                simulator: "ltspice".to_string(),
            }
        }
    }
}

/// Handle cancel request
async fn handle_cancel(request: &CancelRequest, state: &AppState) -> CancelResponse {
    let current_id = state.current_simulation_id.read().await.clone();

    // Check if we're cancelling the right simulation
    let success = if let Some(current) = current_id {
        if current == request.request_id {
            // Set cancel flag
            state.cancel_requested.store(true, Ordering::SeqCst);
            log::info!("Cancel requested for simulation: {}", request.request_id);

            // Try to kill the LTspice process
            let pid = state.current_process_id.load(Ordering::SeqCst);
            if pid != 0 {
                log::info!("Attempting to kill LTspice process with PID: {}", pid);
                kill_process(pid);
            }

            true
        } else {
            log::warn!(
                "Cancel request for {} but current simulation is {}",
                request.request_id,
                current
            );
            false
        }
    } else {
        log::warn!("Cancel request but no simulation running");
        false
    };

    CancelResponse {
        id: uuid::Uuid::new_v4().to_string(),
        msg_type: "cancel_response".to_string(),
        request_id: request.request_id.clone(),
        timestamp: now_ms(),
        success,
    }
}

/// Kill a process by PID
fn kill_process(pid: u32) {
    #[cfg(unix)]
    {
        use std::process::Command;
        // Send SIGTERM first, then SIGKILL
        let _ = Command::new("kill")
            .arg("-15")  // SIGTERM
            .arg(pid.to_string())
            .output();

        // Give it a moment, then force kill
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = Command::new("kill")
            .arg("-9")  // SIGKILL
            .arg(pid.to_string())
            .output();

        log::info!("Sent kill signals to process {}", pid);
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output();

        log::info!("Sent taskkill to process {}", pid);
    }
}
