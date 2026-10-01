mod commands;
mod state;

use commands::*;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,app=debug,pjsua=debug,sip_transport=debug,sip_ua=debug,sip_transaction=debug,media=debug")),
        )
        .try_init();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init_sip_stack,
            register_account,
            make_call,
            answer_call,
            hangup_call,
            send_dtmf,
            set_mute,
            set_speaker_mute,
            set_hold,
            get_audio_devices,
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
