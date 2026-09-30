use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tracing::info;

use pjsua::prelude::{
    AccountConfig, CallState, Endpoint, EndpointConfig, PjsuaEvent, list_audio_devices,
};

use crate::state::AppState;

pub type CmdResult<T> = std::result::Result<T, String>;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AccountConfigDto {
    pub username: String,
    pub password: String,
    pub domain: String,
    pub registrar: String,
    pub expires: u32,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AudioDeviceDto {
    pub name: String,
    pub is_default: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct EndpointInfoDto {
    pub sip_address: String,
    pub is_initialized: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum SipEventPayload {
    #[serde(rename = "incoming_call")]
    IncomingCall {
        call_id: String,
        from_uri: String,
        to_uri: String,
    },
    #[serde(rename = "call_state")]
    CallStateChanged {
        call_id: String,
        state: String,
        status_code: u16,
        reason: String,
    },
    #[serde(rename = "media_ready")]
    MediaReady { call_id: String, codec: String },
    #[serde(rename = "registration_state")]
    RegistrationStateChanged {
        acc_id: usize,
        state: String,
        status_code: u16,
        reason: String,
    },
}

#[tauri::command]
pub async fn init_sip_stack(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    sip_port: Option<u16>,
) -> CmdResult<EndpointInfoDto> {
    let mut current_ep = state.endpoint.lock().await;
    if let Some(ep) = current_ep.as_ref() {
        return Ok(EndpointInfoDto {
            sip_address: ep.local_sip_addr().to_string(),
            is_initialized: true,
        });
    }

    let port = sip_port.unwrap_or(5060);
    let mut config = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
        rtp_port_start: 16000,
        ..Default::default()
    };

    let endpoint = match Endpoint::init(config.clone()).await {
        Ok(ep) => ep,
        Err(_) => {
            // Port 5060 in use, fall back to ephemeral port
            config.sip_bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
            Endpoint::init(config).await.map_err(|e| e.to_string())?
        }
    };

    let local_addr = endpoint.local_sip_addr();
    info!("SIP Endpoint started on {}", local_addr);

    // Background listener translating PjsuaEvents to frontend Tauri events
    let ep_events = Arc::clone(&endpoint);
    let app_handle_clone = app_handle.clone();
    let calls_map = Arc::clone(&state.active_calls);

    tokio::spawn(async move {
        while let Some(event) = ep_events.next_event().await {
            match event {
                PjsuaEvent::IncomingCall {
                    call_id,
                    from_uri,
                    to_uri,
                } => {
                    let _ = app_handle_clone.emit(
                        "sip-event",
                        SipEventPayload::IncomingCall {
                            call_id: call_id.clone(),
                            from_uri,
                            to_uri,
                        },
                    );
                }
                PjsuaEvent::CallStateChanged {
                    call_id,
                    state,
                    status_code,
                    reason,
                } => {
                    let state_str = state.to_string();
                    if state == CallState::Confirmed {
                        if let Some(call) = ep_events.get_call(&call_id).await {
                            call.start_hardware_audio().await;
                        }
                    } else if state == CallState::Disconnected {
                        calls_map.lock().await.remove(&call_id);
                    }

                    let _ = app_handle_clone.emit(
                        "sip-event",
                        SipEventPayload::CallStateChanged {
                            call_id,
                            state: state_str,
                            status_code,
                            reason,
                        },
                    );
                }
                PjsuaEvent::MediaStreamReady { call_id, codec, .. } => {
                    let _ = app_handle_clone
                        .emit("sip-event", SipEventPayload::MediaReady { call_id, codec });
                }
                PjsuaEvent::RegistrationStateChanged {
                    acc_id,
                    state,
                    status_code,
                    reason,
                } => {
                    let _ = app_handle_clone.emit(
                        "sip-event",
                        SipEventPayload::RegistrationStateChanged {
                            acc_id,
                            state: format!("{:?}", state),
                            status_code,
                            reason,
                        },
                    );
                }
            }
        }
    });

    *current_ep = Some(Arc::clone(&endpoint));

    Ok(EndpointInfoDto {
        sip_address: local_addr.to_string(),
        is_initialized: true,
    })
}

#[tauri::command]
pub async fn register_account(
    state: State<'_, AppState>,
    config: AccountConfigDto,
) -> CmdResult<usize> {
    let ep = {
        let lock = state.endpoint.lock().await;
        lock.clone().ok_or("SIP stack not initialized")?
    };

    let registrar_addr: SocketAddr = if config.registrar.contains(':') {
        config
            .registrar
            .parse()
            .map_err(|e| format!("Invalid registrar addr: {}", e))?
    } else {
        format!("{}:5060", config.registrar)
            .parse()
            .map_err(|e| format!("Invalid registrar addr: {}", e))?
    };

    let acc_cfg = AccountConfig {
        username: config.username,
        password: config.password,
        domain: config.domain,
        registrar_addr,
        expires: if config.expires == 0 {
            3600
        } else {
            config.expires
        },
    };

    let acc_id = ep.add_account(acc_cfg).await;
    let _ = ep
        .register_account(acc_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(acc_id)
}

#[tauri::command]
pub async fn make_call(
    state: State<'_, AppState>,
    destination: String,
    account_id: Option<usize>,
) -> CmdResult<String> {
    let ep = {
        let lock = state.endpoint.lock().await;
        lock.clone().ok_or("SIP stack not initialized")?
    };

    let acc_id = account_id.unwrap_or(0);
    let call = ep
        .make_call(acc_id, &destination)
        .await
        .map_err(|e| e.to_string())?;
    let call_id = call.id().to_string();

    state
        .active_calls
        .lock()
        .await
        .insert(call_id.clone(), Arc::clone(&call));

    Ok(call_id)
}

#[tauri::command]
pub async fn answer_call(
    state: State<'_, AppState>,
    call_id: String,
    status_code: Option<u16>,
) -> CmdResult<()> {
    let ep = {
        let lock = state.endpoint.lock().await;
        lock.clone().ok_or("SIP stack not initialized")?
    };

    let code = status_code.unwrap_or(200);
    ep.answer_call(&call_id, code)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(call) = ep.get_call(&call_id).await {
        call.start_hardware_audio().await;
        state.active_calls.lock().await.insert(call_id, call);
    }

    Ok(())
}

#[tauri::command]
pub async fn hangup_call(state: State<'_, AppState>, call_id: String) -> CmdResult<()> {
    let ep = {
        let lock = state.endpoint.lock().await;
        lock.clone().ok_or("SIP stack not initialized")?
    };

    ep.hangup_call(&call_id).await.map_err(|e| e.to_string())?;
    state.active_calls.lock().await.remove(&call_id);
    Ok(())
}

#[tauri::command]
pub async fn send_dtmf(
    state: State<'_, AppState>,
    call_id: String,
    digit: String,
) -> CmdResult<()> {
    let calls = state.active_calls.lock().await;
    let call = calls.get(&call_id).ok_or("Call not active")?;

    if let Some(ch) = digit.chars().next() {
        call.send_dtmf(ch).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_mute(
    state: State<'_, AppState>,
    call_id: String,
    mute_mic: bool,
) -> CmdResult<()> {
    let calls = state.active_calls.lock().await;
    let call = calls.get(&call_id).ok_or("Call not active")?;
    call.mute_mic(mute_mic).await;
    Ok(())
}

#[tauri::command]
pub fn get_audio_devices() -> (Vec<AudioDeviceDto>, Vec<AudioDeviceDto>) {
    let (inputs, outputs) = list_audio_devices();
    let in_dto = inputs
        .into_iter()
        .map(|d| AudioDeviceDto {
            name: d.name,
            is_default: d.is_default,
        })
        .collect();
    let out_dto = outputs
        .into_iter()
        .map(|d| AudioDeviceDto {
            name: d.name,
            is_default: d.is_default,
        })
        .collect();
    (in_dto, out_dto)
}
