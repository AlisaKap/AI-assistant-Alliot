mod actions;
mod assistant;
mod audio_buffer;
mod command_parser;
mod commands;
mod config;
mod voice;
mod voice_state;
mod wake_word;

use std::sync::OnceLock;
use whisper_rs::install_logging_hooks;

pub static VOICE_STATE: OnceLock<voice_state::VoiceStateManager> =
    OnceLock::new();

use crate::voice::{
    start_microphone,
    Microphone,
};

use tauri::Manager;

static MICROPHONE: OnceLock<Microphone> =
    OnceLock::new();


pub fn get_microphone()
    -> Result<&'static Microphone, String>
{
    MICROPHONE
        .get()
        .ok_or_else(|| {
            "Микрофон ещё не инициализирован".to_string()
        })
}


#[cfg_attr(
    mobile,
    tauri::mobile_entry_point
)]
pub fn run() {

    install_logging_hooks();

    tauri::Builder::default()

        // ==================================================
        // SETUP
        // ==================================================

        .setup(|app| {


            // --------------------------------------------------
            // MICROPHONE
            // --------------------------------------------------

            let microphone =
                start_microphone()
                    .map_err(|error| {
                        format!(
                            "Не удалось запустить микрофон: {error}"
                        )
                    })?;


            MICROPHONE
                .set(microphone)
                .map_err(|_| {
                    "Микрофон уже был инициализирован"
                        .to_string()
                })?;


            println!(
                "Микрофон инициализирован"
            );

            VOICE_STATE
                .set(voice_state::VoiceStateManager::new())
                .expect("VOICE_STATE уже был инициализирован");


            // --------------------------------------------------
            // WAKE WORD LISTENER
            // --------------------------------------------------

            crate::voice::start_wake_word_listener(
                app.handle().clone()
            )?;


            println!(
                "[WAKE] Wake word listener автоматически запущен"
            );


            // --------------------------------------------------
            // MAIN WINDOW
            // --------------------------------------------------

            let window =
                app
                    .get_webview_window("main")
                    .ok_or_else(|| {
                        "main window not found".to_string()
                    })?;

            window.set_decorations(false)?;
            window.set_shadow(false)?;

            // --------------------------------------------------
            // MONITOR
            // --------------------------------------------------

            let monitor =
                window
                    .current_monitor()
                    .map_err(|error| {
                        error.to_string()
                    })?
                    .ok_or_else(|| {
                        "no monitor found"
                            .to_string()
                    })?;


            let monitor_size =
                monitor.size();

            let scale_factor =
                monitor.scale_factor();


            // --------------------------------------------------
            // WINDOW SIZE
            // --------------------------------------------------

            let window_width =
                250.0 * scale_factor;

            let window_height =
                300.0 * scale_factor;

            let margin =
                24.0 * scale_factor;


            // --------------------------------------------------
            // BOTTOM RIGHT
            // --------------------------------------------------

            let x =
                monitor_size.width as f64
                    - window_width
                    - margin;

            let y =
                monitor_size.height as f64
                    - window_height
                    - margin;


            window.set_position(
                tauri::Position::Physical(
                    tauri::PhysicalPosition {
                        x: x as i32,
                        y: y as i32,
                    },
                ),
            )?;


            Ok(())
        })


        // ==================================================
        // TAURI COMMANDS
        // ==================================================

        .invoke_handler(
            tauri::generate_handler![
                commands::assistant_process,
                commands::voice_start,
                commands::voice_stop,
                commands::wake_word_start,
                commands::wake_word_stop,
            ],
        )


        // ==================================================
        // RUN
        // ==================================================

        .run(
            tauri::generate_context!()
        )

        .expect(
            "error while running Alliot"
        );
}

