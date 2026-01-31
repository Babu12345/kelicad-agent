// Copyright (c) 2024-2025 Wanyeki Technologies LLC. All rights reserved.
// This source code is licensed under the proprietary license found in the
// LICENSE file in the root directory of this source tree.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod websocket;
mod simulator;
mod protocol;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicBool};
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, State,
};
use tokio::sync::RwLock;

pub struct AppState {
    pub ltspice_path: RwLock<Option<String>>,
    pub is_simulating: RwLock<bool>,
    pub ws_connections: RwLock<u32>,
    pub simulation_count: RwLock<u32>,
    pub last_simulation_time: RwLock<Option<u64>>,
    pub current_simulation_id: RwLock<Option<String>>,
    pub cancel_requested: AtomicBool,
    pub current_process_id: Arc<AtomicU32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ltspice_path: RwLock::new(None),
            is_simulating: RwLock::new(false),
            ws_connections: RwLock::new(0),
            simulation_count: RwLock::new(0),
            last_simulation_time: RwLock::new(None),
            current_simulation_id: RwLock::new(None),
            cancel_requested: AtomicBool::new(false),
            current_process_id: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[derive(Serialize)]
struct AgentStatus {
    ltspice_path: Option<String>,
    ltspice_available: bool,
    is_simulating: bool,
    ws_connections: u32,
    simulation_count: u32,
    last_simulation_time: Option<u64>,
    ws_port: u16,
    version: String,
}

#[tauri::command]
async fn get_agent_status(state: State<'_, Arc<AppState>>) -> Result<AgentStatus, String> {
    let ltspice_path = state.ltspice_path.read().await.clone();
    let is_simulating = *state.is_simulating.read().await;
    let ws_connections = *state.ws_connections.read().await;
    let simulation_count = *state.simulation_count.read().await;
    let last_simulation_time = *state.last_simulation_time.read().await;

    Ok(AgentStatus {
        ltspice_available: ltspice_path.is_some(),
        ltspice_path,
        is_simulating,
        ws_connections,
        simulation_count,
        last_simulation_time,
        ws_port: protocol::WS_PORT,
        version: protocol::AGENT_VERSION.to_string(),
    })
}

fn main() {
    env_logger::init();

    let app_state = Arc::new(AppState::default());
    let ws_state = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![get_agent_status])
        .setup(move |app| {
            // Detect LTspice on startup
            let state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(path) = simulator::detect_ltspice() {
                    let mut ltspice_path = state.ltspice_path.write().await;
                    *ltspice_path = Some(path);
                    log::info!("LTspice detected");
                } else {
                    log::warn!("LTspice not found");
                }
            });

            // Start WebSocket server
            let ws_state_clone = ws_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = websocket::start_server(ws_state_clone).await {
                    log::error!("WebSocket server error: {}", e);
                }
            });

            // Create tray menu
            let quit = MenuItem::with_id(app, "quit", "Quit KeliCAD Agent", true, None::<&str>)?;
            let status = MenuItem::with_id(app, "status", "Status: Ready", false, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status, &quit])?;

            // Create tray icon - use default icon if available, otherwise skip tray icon setup
            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });

            // Only set icon if available
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            log::info!("KeliCAD Agent started on port 9347");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                // Keep running in background when window is closed
                api.prevent_exit();
            }
        });
}
