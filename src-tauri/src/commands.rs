use tauri::AppHandle;

use std::sync::atomic::{
    AtomicBool,
    Ordering,
};

use crate::assistant;
use crate::assistant::AssistantAction;


// ============================================================
// VOICE SESSION STATE
// ============================================================

/// Защищает от одновременного запуска нескольких voice_start.
///
/// Если frontend случайно отправит два запроса подряд,
/// второй не создаст вторую запись поверх первой.
static VOICE_SESSION_RUNNING: AtomicBool =
    AtomicBool::new(false);


// ============================================================
// TEXT COMMAND
// ============================================================

/// Обрабатывает уже распознанную текстовую команду.
#[tauri::command]
pub fn assistant_process(
    text: String,
) -> String {

    process_voice_command(
        &text
    )
}


// ============================================================
// VOICE START
// ============================================================

/// Запускает новую ручную голосовую сессию.
///
/// Каждый вызов:
///
/// 1. проверяет предыдущую сессию;
/// 2. принудительно очищает старое состояние;
/// 3. начинает новую запись;
/// 4. ждёт окончания речи через VAD;
/// 5. получает аудио;
/// 6. отправляет аудио в Whisper;
/// 7. выполняет команду.
#[tauri::command]
pub fn voice_start()
    -> Result<String, String>
{
    // ========================================================
    // ЗАЩИТА ОТ ПОВТОРНОГО ЗАПУСКА
    // ========================================================

    if VOICE_SESSION_RUNNING.swap(
        true,
        Ordering::SeqCst,
    ) {

        println!(
            "voice_start: предыдущая voice-сессия ещё работает"
        );

        return Err(
            "Предыдущая голосовая сессия ещё выполняется"
                .to_string()
        );
    }


    // ========================================================
    // ГАРАНТИРОВАННО СБРОСИТЬ ФЛАГ
    // ========================================================

    struct SessionGuard;

    impl Drop for SessionGuard {

        fn drop(&mut self) {

            VOICE_SESSION_RUNNING.store(
                false,
                Ordering::SeqCst,
            );

            println!(
                "voice_start: voice-сессия освобождена"
            );
        }
    }


    let _guard =
        SessionGuard;


    // ========================================================
    // MICROPHONE
    // ========================================================

    let microphone =
        crate::get_microphone()?;


    // ========================================================
    // ПРОВЕРКА СТАРОЙ ЗАПИСИ
    // ========================================================

    if microphone.is_recording() {

        println!(
            "voice_start: обнаружена старая запись, останавливаем"
        );


        let _ =
            microphone.stop_recording();


        // Даём CPAL callback немного времени
        // завершить текущий цикл.
        std::thread::sleep(
            std::time::Duration::from_millis(50)
        );
    }


    // ========================================================
    // START
    // ========================================================

    microphone.start_recording()?;


    println!(
        "voice_start: НОВАЯ запись начата"
    );


    // ========================================================
    // WAIT FOR SPEECH END
    // ========================================================
    //
    // Здесь происходит реальная запись.
    //
    // wait_for_recording_end():
    //
    // - ждёт начала речи;
    // - ждёт окончания речи;
    // - отслеживает VAD;
    // - останавливает запись после 1 сек тишины;
    // - ограничивает запись 7 секундами;
    // - возвращает записанное аудио.
    //
    // ========================================================

    let audio =
        microphone.wait_for_recording_end()?;


    println!(
        "voice_start: запись завершена, {} samples",
        audio.len()
    );


    // ========================================================
    // EMPTY AUDIO
    // ========================================================

    if audio.is_empty() {

        return Err(
            "Нет записанного аудио".to_string()
        );
    }


    // ========================================================
    // WHISPER
    // ========================================================

    let text =
        crate::voice::transcribe(
            &audio,
            microphone.sample_rate(),
        )?;


    println!(
        "voice_start: Whisper -> {}",
        text
    );


    // ========================================================
    // COMMAND
    // ========================================================

    let response =
        process_voice_command(
            &text
        );


    println!(
        "voice_start: действие -> {}",
        response
    );


    Ok(response)
}


// ============================================================
// VOICE STOP
// ============================================================


#[tauri::command]
pub fn voice_stop()
    -> Result<String, String>
{
    let microphone =
        crate::get_microphone()?;


    if !microphone.is_recording() {

        return Err(
            "Сейчас нет активной записи"
                .to_string()
        );
    }


    let audio =
        microphone.stop_recording()?;


    println!(
        "voice_stop: запись остановлена, {} samples",
        audio.len()
    );


    if audio.is_empty() {

        return Err(
            "Нет записанного аудио"
                .to_string()
        );
    }


    let result =
        crate::voice::transcribe(
            &audio,
            microphone.sample_rate(),
        )?;


    println!(
        "voice_stop: Whisper -> {}",
        result
    );


    Ok(result)
}


// ============================================================
// WAKE WORD START
// ============================================================

/// Запускает постоянное прослушивание wake word.
#[tauri::command]
pub fn wake_word_start(
    app: AppHandle,
) -> Result<(), String> {

    crate::voice::start_wake_word_listener(
        app
    )?;

    Ok(())
}

// ============================================================
// WAKE WORD STOP
// ============================================================

/// Останавливает постоянное прослушивание wake word.
#[tauri::command]
pub fn wake_word_stop()
    -> Result<String, String>
{
    crate::voice::stop_wake_word_listener()?;


    Ok(
        "Wake word listener остановлен".to_string()
    )
}


// ============================================================
// COMMAND PROCESSING
// ============================================================

/// Передаёт распознанный текст ассистенту
/// и выполняет соответствующее действие.
pub fn process_voice_command(
    text: &str,
) -> String {

    println!(
        "process_voice_command: {}",
        text
    );


    match assistant::process_command(text) {

        // ====================================================
        // RESPONSE
        // ====================================================

        AssistantAction::Respond(
            response
        ) => {

            response
        }


        // ====================================================
        // OPEN APPLICATION
        // ====================================================

        AssistantAction::OpenApplication(
            application
        ) => {

            match crate::actions::open_application(
                &application
            ) {

                Ok(()) => {

                    format!(
                        "Открываю: {}",
                        application
                    )
                }

                Err(error) => {

                    format!(
                        "Ошибка открытия {}: {}",
                        application,
                        error
                    )
                }
            }
        }


        // ====================================================
        // OPEN WORKSPACE
        // ====================================================

        AssistantAction::OpenWorkspace => {

            match crate::actions::open_workspace() {

                Ok(()) => {

                    "Щиты подняты".to_string()
                }

                Err(error) => {

                    format!(
                        "Ошибка workspace: {}",
                        error
                    )
                }
            }
        }


        // ====================================================
        // UNKNOWN
        // ====================================================

        AssistantAction::Unknown(
            command
        ) => {

            format!(
                "Я пока не знаю такую команду: {}",
                command
            )
        }
    }
}
