mod actions;
mod assistant;
mod audio_buffer;
mod command_parser;
mod commands;
mod config;
mod voice;
mod wake_word;

use std::sync::OnceLock;

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
                        "main window not found"
                            .to_string()
                    })?;


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
                360.0 * scale_factor;

            let window_height =
                500.0 * scale_factor;

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