use crate::audio_buffer::AudioRingBuffer;

use cpal::traits::{
    DeviceTrait,
    HostTrait,
    StreamTrait,
};

use std::sync::{
    Arc,
    Mutex,
    OnceLock,
};

use std::sync::atomic::{
    AtomicBool,
    Ordering,
};

use std::thread;
use std::time::Duration;

use tauri::{
    AppHandle,
    Emitter,
};

use whisper_rs::{
    FullParams,
    SamplingStrategy,
    WhisperContext,
    WhisperContextParameters,
};


// ============================================================
// SETTINGS
// ============================================================

const MODEL_PATH: &str =
    "models/ggml-small.bin";

const WHISPER_SAMPLE_RATE: u32 =
    16_000;


// ============================================================
// WAKE WORD
// ============================================================

const WAKE_WORD_WINDOW_SECONDS: f32 =
    3.0;

const WAKE_WORD_INTERVAL_MS: u64 =
    700;

const WAKE_WORD_SILENCE_THRESHOLD: f32 =
    0.009;


// ============================================================
// COMMAND
// ============================================================

const MAX_COMMAND_SECONDS: f32 =
    7.0;

const COMMAND_SILENCE_SECONDS: f32 =
    1.0;

const SILENCE_THRESHOLD: f32 =
    0.02;

const MIN_SPEECH_SECONDS: f32 =
    0.10;


// ============================================================
// RING BUFFER
// ============================================================

const RING_BUFFER_SECONDS: usize =
    5;


// ============================================================
// WHISPER
// ============================================================

static WHISPER_CONTEXT:
    OnceLock<Result<WhisperContext, String>> =
    OnceLock::new();


// ============================================================
// ASSISTANT STATE
// ============================================================

static ASSISTANT_AWAKE:
    AtomicBool =
    AtomicBool::new(false);

static WAKE_WORD_LISTENER_RUNNING:
    AtomicBool =
    AtomicBool::new(false);


// ============================================================
// MICROPHONE
// ============================================================

pub struct Microphone {

    buffer:
        Arc<Mutex<AudioRingBuffer>>,

    recording_buffer:
        Arc<Mutex<Vec<f32>>>,

    recording:
        Arc<AtomicBool>,

    auto_stop:
        Arc<AtomicBool>,

    speech_started:
        Arc<AtomicBool>,

    speech_samples:
        Arc<Mutex<usize>>,

    silence_samples:
        Arc<Mutex<usize>>,

    sample_rate:
        u32,
}


// ============================================================
// MICROPHONE METHODS
// ============================================================

impl Microphone {

    // ========================================================
    // INFO
    // ========================================================

    pub fn sample_rate(
        &self,
    ) -> u32 {

        self.sample_rate
    }


    pub fn buffer(
        &self,
    ) -> Arc<Mutex<AudioRingBuffer>> {

        Arc::clone(
            &self.buffer
        )
    }


    pub fn recent_audio(
        &self,
    ) -> Result<Vec<f32>, String> {

        let buffer =
            self.buffer
                .lock()
                .map_err(|_| {
                    "Не удалось получить аудиобуфер".to_string()
                })?;


        Ok(
            buffer.snapshot()
        )
    }


    pub fn clear_audio(
        &self,
    ) -> Result<(), String> {

        let mut buffer =
            self.buffer
                .lock()
                .map_err(|_| {
                    "Не удалось получить аудиобуфер".to_string()
                })?;


        buffer.clear();


        Ok(())
    }


    // ========================================================
    // START RECORDING
    // ========================================================

    pub fn start_recording(
        &self,
    ) -> Result<(), String> {

        self.recording.store(
            false,
            Ordering::SeqCst,
        );

        self.auto_stop.store(
            false,
            Ordering::SeqCst,
        );

        self.speech_started.store(
            false,
            Ordering::SeqCst,
        );


        // ----------------------------------------------------
        // CLEAR RECORDING BUFFER
        // ----------------------------------------------------

        {
            let mut buffer =
                self.recording_buffer
                    .lock()
                    .map_err(|_| {
                        "Не удалось очистить буфер записи".to_string()
                    })?;


            buffer.clear();
        }


        // ----------------------------------------------------
        // RESET VAD
        // ----------------------------------------------------

        {
            let mut value =
                self.speech_samples
                    .lock()
                    .map_err(|_| {
                        "Ошибка состояния речи".to_string()
                    })?;


            *value = 0;
        }


        {
            let mut value =
                self.silence_samples
                    .lock()
                    .map_err(|_| {
                        "Ошибка состояния тишины".to_string()
                    })?;


            *value = 0;
        }


        // ----------------------------------------------------
        // START
        // ----------------------------------------------------

        self.recording.store(
            true,
            Ordering::SeqCst,
        );


        println!(
            "[VOICE] recording = ON"
        );


        Ok(())
    }


    // ========================================================
    // STOP RECORDING
    // ========================================================

    pub fn stop_recording(
        &self,
    ) -> Result<Vec<f32>, String> {

        self.recording.store(
            false,
            Ordering::SeqCst,
        );

        self.auto_stop.store(
            false,
            Ordering::SeqCst,
        );


        let audio =
            self.recording_buffer
                .lock()
                .map_err(|_| {
                    "Не удалось получить запись".to_string()
                })?
                .clone();


        println!(
            "[VOICE] recording = OFF, {} samples",
            audio.len()
        );


        Ok(audio)
    }


    // ========================================================
    // WAIT RECORDING END
    // ========================================================

    pub fn wait_for_recording_end(
        &self,
    ) -> Result<Vec<f32>, String> {

        loop {

            if !self.is_recording() {
                break;
            }


            if self.should_auto_stop() {

                self.recording.store(
                    false,
                    Ordering::SeqCst,
                );

                break;
            }


            thread::sleep(
                Duration::from_millis(20)
            );
        }


        self.stop_recording()
    }


    // ========================================================
    // STATE
    // ========================================================

    pub fn is_recording(
        &self,
    ) -> bool {

        self.recording.load(
            Ordering::Relaxed
        )
    }


    pub fn should_auto_stop(
        &self,
    ) -> bool {

        self.auto_stop.load(
            Ordering::Relaxed
        )
    }
}


// ============================================================
// START MICROPHONE
// ============================================================

pub fn start_microphone()
    -> Result<Microphone, String>
{

    if let Some(state) = crate::VOICE_STATE.get() {
        state.set(crate::voice_state::VoiceState::WaitingForWakeWord);
    }

    let host =
        cpal::default_host();


    let device =
        host
            .default_input_device()
            .ok_or_else(|| {
                "Микрофон не найден".to_string()
            })?;


    let device_name =
        device
            .name()
            .unwrap_or_else(|_| {
                "Unknown".to_string()
            });


    println!(
        "[MIC] device: {}",
        device_name
    );


    let supported_config =
        device
            .default_input_config()
            .map_err(|error| {
                format!(
                    "Не удалось получить конфигурацию микрофона: {error}"
                )
            })?;


    let sample_format =
        supported_config.sample_format();


    let config:
        cpal::StreamConfig =
        supported_config.into();


    if config.channels != 1 {

        return Err(
            format!(
                "Нужен mono-микрофон. Получено каналов: {}",
                config.channels
            )
        );
    }


    let sample_rate =
        config.sample_rate.0;


    println!(
        "[MIC] {} Hz / {} channel",
        sample_rate,
        config.channels
    );


    // ========================================================
    // BUFFERS
    // ========================================================

    let ring_capacity =
        sample_rate as usize
            * RING_BUFFER_SECONDS;


    let buffer =
        Arc::new(
            Mutex::new(
                AudioRingBuffer::new(
                    ring_capacity
                )
            )
        );


    let recording_buffer =
        Arc::new(
            Mutex::new(
                Vec::<f32>::new()
            )
        );


    let recording =
        Arc::new(
            AtomicBool::new(false)
        );


    let auto_stop =
        Arc::new(
            AtomicBool::new(false)
        );


    let speech_started =
        Arc::new(
            AtomicBool::new(false)
        );


    let speech_samples =
        Arc::new(
            Mutex::new(0usize)
        );


    let silence_samples =
        Arc::new(
            Mutex::new(0usize)
        );


    // ========================================================
    // THREAD REFERENCES
    // ========================================================

    let buffer_thread =
        Arc::clone(&buffer);

    let recording_buffer_thread =
        Arc::clone(&recording_buffer);

    let recording_thread =
        Arc::clone(&recording);

    let auto_stop_thread =
        Arc::clone(&auto_stop);

    let speech_started_thread =
        Arc::clone(&speech_started);

    let speech_samples_thread =
        Arc::clone(&speech_samples);

    let silence_samples_thread =
        Arc::clone(&silence_samples);


    // ========================================================
    // AUDIO THREAD
    // ========================================================

    thread::spawn(move || {

        let err_fn =
            |error| {

                eprintln!(
                    "[MIC] audio error: {}",
                    error
                );
            };


        let stream_result =
            match sample_format {

                // ==================================================
                // F32
                // ==================================================

                cpal::SampleFormat::F32 => {

                    let buffer =
                        Arc::clone(
                            &buffer_thread
                        );

                    let recording_buffer =
                        Arc::clone(
                            &recording_buffer_thread
                        );

                    let recording =
                        Arc::clone(
                            &recording_thread
                        );

                    let auto_stop =
                        Arc::clone(
                            &auto_stop_thread
                        );

                    let speech_started =
                        Arc::clone(
                            &speech_started_thread
                        );

                    let speech_samples =
                        Arc::clone(
                            &speech_samples_thread
                        );

                    let silence_samples =
                        Arc::clone(
                            &silence_samples_thread
                        );


                    device.build_input_stream(

                        &config,

                        move |data: &[f32], _| {

                            process_audio(
                                data,
                                sample_rate,
                                &buffer,
                                &recording_buffer,
                                &recording,
                                &auto_stop,
                                &speech_started,
                                &speech_samples,
                                &silence_samples,
                            );
                        },

                        err_fn,
                        None,
                    )
                }


                // ==================================================
                // I16
                // ==================================================

                cpal::SampleFormat::I16 => {

                    let buffer =
                        Arc::clone(
                            &buffer_thread
                        );

                    let recording_buffer =
                        Arc::clone(
                            &recording_buffer_thread
                        );

                    let recording =
                        Arc::clone(
                            &recording_thread
                        );

                    let auto_stop =
                        Arc::clone(
                            &auto_stop_thread
                        );

                    let speech_started =
                        Arc::clone(
                            &speech_started_thread
                        );

                    let speech_samples =
                        Arc::clone(
                            &speech_samples_thread
                        );

                    let silence_samples =
                        Arc::clone(
                            &silence_samples_thread
                        );


                    device.build_input_stream(

                        &config,

                        move |data: &[i16], _| {

                            let samples =
                                convert_i16_to_f32(
                                    data
                                );


                            process_audio(
                                &samples,
                                sample_rate,
                                &buffer,
                                &recording_buffer,
                                &recording,
                                &auto_stop,
                                &speech_started,
                                &speech_samples,
                                &silence_samples,
                            );
                        },

                        err_fn,
                        None,
                    )
                }


                // ==================================================
                // U16
                // ==================================================

                cpal::SampleFormat::U16 => {

                    let buffer =
                        Arc::clone(
                            &buffer_thread
                        );

                    let recording_buffer =
                        Arc::clone(
                            &recording_buffer_thread
                        );

                    let recording =
                        Arc::clone(
                            &recording_thread
                        );

                    let auto_stop =
                        Arc::clone(
                            &auto_stop_thread
                        );

                    let speech_started =
                        Arc::clone(
                            &speech_started_thread
                        );

                    let speech_samples =
                        Arc::clone(
                            &speech_samples_thread
                        );

                    let silence_samples =
                        Arc::clone(
                            &silence_samples_thread
                        );


                    device.build_input_stream(

                        &config,

                        move |data: &[u16], _| {

                            let samples =
                                convert_u16_to_f32(
                                    data
                                );


                            process_audio(
                                &samples,
                                sample_rate,
                                &buffer,
                                &recording_buffer,
                                &recording,
                                &auto_stop,
                                &speech_started,
                                &speech_samples,
                                &silence_samples,
                            );
                        },

                        err_fn,
                        None,
                    )
                }


                format => {

                    eprintln!(
                        "[MIC] unsupported format: {:?}",
                        format
                    );

                    return;
                }
            };


        let stream =
            match stream_result {

                Ok(stream) =>
                    stream,

                Err(error) => {

                    eprintln!(
                        "[MIC] stream error: {}",
                        error
                    );

                    return;
                }
            };


        if let Err(error) =
            stream.play()
        {

            eprintln!(
                "[MIC] play error: {}",
                error
            );

            return;
        }


        println!(
            "[MIC] микрофон постоянно слушает: {} Hz",
            sample_rate
        );


        loop {

            thread::sleep(
                Duration::from_secs(60)
            );
        }
    });


    Ok(
        Microphone {
            buffer,
            recording_buffer,
            recording,
            auto_stop,
            speech_started,
            speech_samples,
            silence_samples,
            sample_rate,
        }
    )
}


// ============================================================
// AUDIO PROCESSING
// ============================================================

fn process_audio(
    samples: &[f32],
    sample_rate: u32,

    buffer:
        &Arc<Mutex<AudioRingBuffer>>,

    recording_buffer:
        &Arc<Mutex<Vec<f32>>>,

    recording:
        &Arc<AtomicBool>,

    auto_stop:
        &Arc<AtomicBool>,

    speech_started:
        &Arc<AtomicBool>,

    speech_samples:
        &Arc<Mutex<usize>>,

    silence_samples:
        &Arc<Mutex<usize>>,
) {

    // ========================================================
    // RING BUFFER
    // ========================================================

    if let Ok(
        mut buffer
    ) = buffer.lock()
    {
        buffer.push(
            samples
        );
    }


    // ========================================================
    // RECORDING OFF
    // ========================================================

    if !recording.load(
        Ordering::Relaxed
    ) {
        return;
    }


    // ========================================================
    // SAVE RECORDING
    // ========================================================

    if let Ok(
        mut buffer
    ) = recording_buffer.lock()
    {
        buffer.extend_from_slice(
            samples
        );
    }


    // ========================================================
    // RECORDING LENGTH
    // ========================================================

    let recorded_samples =
        match recording_buffer.lock()
        {

            Ok(buffer) =>
                buffer.len(),

            Err(_) =>
                return,
        };


    let recorded_seconds =
        recorded_samples as f32
            / sample_rate as f32;


    if recorded_seconds
        >= MAX_COMMAND_SECONDS
    {

        println!(
            "[VOICE] достигнут максимум {:.1}s",
            MAX_COMMAND_SECONDS
        );


        auto_stop.store(
            true,
            Ordering::SeqCst,
        );


        return;
    }


    // ========================================================
    // VAD
    // ========================================================

    let volume =
        calculate_rms(
            samples
        );


    let is_speech =
        volume >= SILENCE_THRESHOLD;


    // ========================================================
    // SPEECH START
    // ========================================================

    if !speech_started.load(
        Ordering::Relaxed
    ) {

        if is_speech {

            speech_started.store(
                true,
                Ordering::SeqCst,
            );


            if let Ok(
                mut value
            ) = speech_samples.lock()
            {
                *value =
                    samples.len();
            }


            if let Ok(
                mut value
            ) = silence_samples.lock()
            {
                *value = 0;
            }


            println!(
                "[VOICE] речь началась"
            );
        }


        return;
    }


    // ========================================================
    // SPEECH CONTINUES
    // ========================================================

    if is_speech {

        if let Ok(
            mut value
        ) = speech_samples.lock()
        {
            *value +=
                samples.len();
        }


        if let Ok(
            mut value
        ) = silence_samples.lock()
        {
            *value = 0;
        }


        return;
    }


    // ========================================================
    // SILENCE
    // ========================================================

    let silence_count =
        match silence_samples.lock()
        {

            Ok(mut value) => {

                *value +=
                    samples.len();

                *value
            }

            Err(_) =>
                return,
        };


    let silence_seconds =
        silence_count as f32
            / sample_rate as f32;


    if silence_seconds
        < COMMAND_SILENCE_SECONDS
    {
        return;
    }


    // ========================================================
    // SPEECH LENGTH
    // ========================================================

    let speech_count =
        match speech_samples.lock()
        {

            Ok(value) =>
                *value,

            Err(_) =>
                return,
        };


    let speech_seconds =
        speech_count as f32
            / sample_rate as f32;


    if speech_seconds
        >= MIN_SPEECH_SECONDS
    {

        println!(
            "[VOICE] конец речи: {:.2}s",
            speech_seconds
        );


        auto_stop.store(
            true,
            Ordering::SeqCst,
        );
    }
}


// ============================================================
// WAKE WORD LISTENER
// ============================================================

pub fn start_wake_word_listener(
    app: AppHandle,
) -> Result<(), String>
{

    // ========================================================
    // PREVENT SECOND LISTENER
    // ========================================================

    if WAKE_WORD_LISTENER_RUNNING.swap(
        true,
        Ordering::SeqCst,
    ) {

        println!(
            "[WAKE] listener уже работает"
        );

        return Ok(());
    }


    // ========================================================
    // GET MICROPHONE
    // ========================================================

    let microphone =
        match crate::get_microphone()
        {

            Ok(microphone) =>
                microphone,

            Err(error) => {

                WAKE_WORD_LISTENER_RUNNING.store(
                    false,
                    Ordering::SeqCst,
                );

                return Err(error);
            }
        };


    let buffer =
        microphone.buffer();


    let sample_rate =
        microphone.sample_rate();


    thread::spawn(move || {

        println!(
            "[WAKE] listener запущен"
        );

        println!(
            "[WAKE] ожидаю имя: \"Аллиот\""
        );


        let window_samples =
            (
                sample_rate as f32
                    * WAKE_WORD_WINDOW_SECONDS
            ) as usize;


        while WAKE_WORD_LISTENER_RUNNING.load(
            Ordering::Relaxed
        ) {

            thread::sleep(
                Duration::from_millis(
                    WAKE_WORD_INTERVAL_MS
                )
            );


            // ==================================================
            // ASSISTANT ACTIVE
            // ==================================================

            if ASSISTANT_AWAKE.load(
                Ordering::Relaxed
            ) {
                continue;
            }


            // ==================================================
            // RECORDING ACTIVE
            // ==================================================

            if microphone.is_recording() {
                continue;
            }


            // ==================================================
            // GET AUDIO
            // ==================================================

            let audio =
                match buffer.lock()
                {

                    Ok(buffer) =>
                        buffer.last_samples(
                            window_samples
                        ),

                    Err(_) =>
                        continue,
                };


            if audio.len()
                < window_samples / 2
            {
                continue;
            }


            // ==================================================
            // RECENT VAD
            // ==================================================

            let recent_samples =
                (
                    sample_rate as f32
                        * 0.5
                ) as usize;


            let start =
                audio.len()
                    .saturating_sub(
                        recent_samples
                    );


            let volume =
                calculate_rms(
                    &audio[start..]
                );


            if volume
                < WAKE_WORD_SILENCE_THRESHOLD
            {
                continue;
            }


            println!(
                "[WAKE] речь обнаружена, проверяю..."
            );


            // ==================================================
            // TRANSCRIBE
            // ==================================================

            let text =
                match transcribe(
                    &audio,
                    sample_rate,
                )
                {

                    Ok(text) =>
                        text,

                    Err(error) => {

                        println!(
                            "[WAKE] Whisper error: {}",
                            error
                        );

                        continue;
                    }
                };


            println!(
                "[WAKE] Whisper: {}",
                text
            );


            // ==================================================
            // FIND WAKE WORD
            // ==================================================

            if let Some(state) = crate::VOICE_STATE.get() {
                state.set(crate::voice_state::VoiceState::Listening);
            }

            let command =
                match crate::wake_word::extract_command(
                    &text
                ) {

                    Some(command) =>
                        command,

                    None =>
                        continue,
                };


            println!(
                "[WAKE] АЛЛИОТ ОБНАРУЖЕН"
            );


            ASSISTANT_AWAKE.store(
                true,
                Ordering::SeqCst,
            );


            // ==================================================
            // CLEAR OLD AUDIO
            // ==================================================

            if let Ok(
                mut buffer
            ) = buffer.lock()
            {
                buffer.clear();
            }


            // ==================================================
            // COMMAND IN SAME PHRASE
            // ==================================================

            if !command.trim().is_empty() {

                println!(
                    "[WAKE] команда: {}",
                    command
                );


                // ------------------------------------------------
                // UI: WAKE
                // ------------------------------------------------

                emit_event(
                    &app,
                    "wake-word-detected",
                );


                // ------------------------------------------------
                // UI: ANALYZING
                // ------------------------------------------------

                emit_event(
                    &app,
                    "voice-analyzing",
                );


                execute_voice_command(
                    &command
                );


                // ------------------------------------------------
                // UI: IDLE
                // ------------------------------------------------

                emit_event(
                    &app,
                    "voice-idle",
                );


                ASSISTANT_AWAKE.store(
                    false,
                    Ordering::SeqCst,
                );


                println!(
                    "[WAKE] снова ожидаю \"Аллиот\""
                );


                continue;
            }


            // ==================================================
            // ONLY WAKE WORD
            // ==================================================

            println!(
                "[WAKE] Аллиот активирован"
            );

            println!(
                "[WAKE] жду команду..."
            );


            // ==================================================
            // UI: WAKE
            // ==================================================

            emit_event(
                &app,
                "wake-word-detected",
            );


            // ==================================================
            // LISTEN FOR COMMAND
            // ==================================================

            match listen_for_command(
                &microphone,
                &app,
            ) {

                Ok(command) => {

                    if !command.trim().is_empty() {

                        execute_voice_command(
                            &command
                        );
                    }
                }

                Err(error) => {

                    println!(
                        "[WAKE] команда не получена: {}",
                        error
                    );
                }
            }


            // ==================================================
            // UI: IDLE
            // ==================================================

            if let Some(state) = crate::VOICE_STATE.get() {
                state.set(crate::voice_state::VoiceState::Analyzing);
            }

            emit_event(
                &app,
                "voice-idle",
            );


            // ==================================================
            // RETURN TO WAKE MODE
            // ==================================================

            ASSISTANT_AWAKE.store(
                false,
                Ordering::SeqCst,
            );


            println!(
                "[WAKE] снова ожидаю \"Аллиот\""
            );
        }


        println!(
            "[WAKE] listener остановлен"
        );
    });


    Ok(())
}


// ============================================================
// STOP WAKE WORD LISTENER
// ============================================================

pub fn stop_wake_word_listener()
    -> Result<(), String>
{

    WAKE_WORD_LISTENER_RUNNING.store(
        false,
        Ordering::SeqCst,
    );


    ASSISTANT_AWAKE.store(
        false,
        Ordering::SeqCst,
    );


    println!(
        "[WAKE] listener остановлен"
    );


    Ok(())
}


// ============================================================
// COMMAND LISTENER
// ============================================================

fn listen_for_command(
    microphone: &Microphone,
    app: &AppHandle,
) -> Result<String, String>
{

    println!(
        "[COMMAND] слушаю..."
    );


    // ========================================================
    // UI: LISTENING
    // ========================================================

    emit_event(
        app,
        "voice-listening",
    );


    // ========================================================
    // START RECORDING
    // ========================================================

    microphone.start_recording()?;


    // ========================================================
    // WAIT
    // ========================================================

    let audio =
        microphone.wait_for_recording_end()?;


    if audio.is_empty() {

        return Err(
            "Команда не содержит аудио".to_string()
        );
    }


    println!(
        "[COMMAND] получено {} samples",
        audio.len()
    );


    // ========================================================
    // UI: ANALYZING
    // ========================================================

    emit_event(
        app,
        "voice-analyzing",
    );


    // ========================================================
    // TRANSCRIBE
    // ========================================================

    let text =
        transcribe(
            &audio,
            microphone.sample_rate(),
        )?;


    println!(
        "[COMMAND] Whisper: {}",
        text
    );


    // ========================================================
    // REMOVE WAKE WORD
    // ========================================================

    let command =
        crate::wake_word::remove_wake_word(
            &text
        );


    Ok(command)
}


// ============================================================
// UI EVENT
// ============================================================

fn emit_event(
    app: &AppHandle,
    event: &str,
) {

    if let Err(error) =
        app.emit(
            event,
            ()
        )
    {

        eprintln!(
            "[UI] ошибка события {}: {}",
            event,
            error
        );

    } else {

        println!(
            "[UI] {} отправлено",
            event
        );
    }
}


// ============================================================
// EXECUTE COMMAND
// ============================================================

fn execute_voice_command(
    text: &str,
) {

    let command =
        text.trim();


    if command.is_empty() {
        return;
    }


    println!(
        "[ASSISTANT] command: {}",
        command
    );


    let result =
        crate::assistant::process_command(
            command
        );


    match result {

        crate::assistant::AssistantAction::Respond(
            response
        ) => {

            println!(
                "Alliot: {}",
                response
            );
        }


        crate::assistant::AssistantAction::OpenApplication(
            application
        ) => {

            match crate::actions::open_application(
                &application
            ) {

                Ok(()) => {

                    println!(
                        "Открываю: {}",
                        application
                    );
                }


                Err(error) => {

                    eprintln!(
                        "Ошибка открытия {}: {}",
                        application,
                        error
                    );
                }
            }
        }


        crate::assistant::AssistantAction::OpenWorkspace => {

            match crate::actions::open_workspace() {

                Ok(()) => {

                    println!(
                        "Workspace открыт"
                    );
                }


                Err(error) => {

                    eprintln!(
                        "Ошибка workspace: {}",
                        error
                    );
                }
            }
        }


        crate::assistant::AssistantAction::Unknown(
            command
        ) => {

            println!(
                "Неизвестная команда: {}",
                command
            );
        }
    }
}


// ============================================================
// WHISPER CONTEXT
// ============================================================

fn get_whisper_context()
    -> Result<&'static WhisperContext, String>
{

    let context =
        WHISPER_CONTEXT
            .get_or_init(|| {

                println!(
                    "[WHISPER] загрузка модели..."
                );


                whisper_rs::print_system_info();


                let mut params =
                    WhisperContextParameters::default();


                params.use_gpu =
                    true;


                params.gpu_device =
                    0;


                WhisperContext::new_with_params(
                    MODEL_PATH,
                    params,
                )
                .map_err(|error| {

                    format!(
                        "Не удалось загрузить Whisper: {}",
                        error
                    )
                })
            });


    context
        .as_ref()
        .map_err(|error| error.clone())
}


// ============================================================
// TRANSCRIBE
// ============================================================

pub fn transcribe(
    audio: &[f32],
    input_sample_rate: u32,
) -> Result<String, String>
{

    if audio.is_empty() {

        return Err(
            "Аудио пустое".to_string()
        );
    }


    if input_sample_rate == 0 {

        return Err(
            "Некорректная частота дискретизации"
                .to_string()
        );
    }


    let duration =
        audio.len() as f32
            / input_sample_rate as f32;


    if duration
        < MIN_SPEECH_SECONDS
    {

        return Err(
            format!(
                "Слишком короткое аудио: {:.2}s",
                duration
            )
        );
    }


    // ========================================================
    // RESAMPLE
    // ========================================================

    let audio_16k =
        resample_to_16khz(
            audio,
            input_sample_rate,
        );


    if audio_16k.is_empty() {

        return Err(
            "После resample аудио пустое"
                .to_string()
        );
    }


    // ========================================================
    // WHISPER
    // ========================================================

    let context =
        get_whisper_context()?;


    let mut state =
        context
            .create_state()
            .map_err(|error| {

                format!(
                    "Не удалось создать Whisper state: {}",
                    error
                )
            })?;


    let mut params =
        FullParams::new(
            SamplingStrategy::Greedy {
                best_of: 1,
            }
        );


    params.set_language(
        Some("ru")
    );


    params.set_translate(
        false
    );


    params.set_no_context(
        true
    );


    params.set_print_special(
        false
    );


    params.set_print_progress(
        false
    );


    params.set_print_realtime(
        false
    );


    params.set_print_timestamps(
        false
    );


    params.set_no_speech_thold(
        0.6
    );


    state
        .full(
            params,
            &audio_16k,
        )
        .map_err(|error| {

            format!(
                "Ошибка Whisper: {}",
                error
            )
        })?;


    // ========================================================
    // COLLECT RESULT
    // ========================================================

    let segment_count =
        state.full_n_segments();


    let mut result =
        String::new();


    for index in 0..segment_count {

        if let Some(segment) =
            state.get_segment(index)
        {

            let text =
                segment
                    .to_str()
                    .map_err(
                        |error|
                            error.to_string()
                    )?;


            result.push_str(
                text
            );


            result.push(' ');
        }
    }


    let result =
        normalize_transcript(
            &result
        );


    if result.is_empty() {

        return Err(
            "Whisper не распознал речь"
                .to_string()
        );
    }


    Ok(result)
}


// ============================================================
// NORMALIZE TRANSCRIPT
// ============================================================

fn normalize_transcript(
    text: &str,
) -> String {

    text
        .to_lowercase()
        .replace('ё', "е")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}


// ============================================================
// RESAMPLE
// ============================================================

fn resample_to_16khz(
    audio: &[f32],
    input_sample_rate: u32,
) -> Vec<f32>
{

    if audio.is_empty()
        || input_sample_rate == 0
    {
        return Vec::new();
    }


    if input_sample_rate
        == WHISPER_SAMPLE_RATE
    {
        return audio.to_vec();
    }


    let ratio =
        input_sample_rate as f64
            / WHISPER_SAMPLE_RATE as f64;


    let output_len =
        (
            audio.len() as f64
                / ratio
        ) as usize;


    let mut output =
        Vec::with_capacity(
            output_len
        );


    for index in 0..output_len {

        let position =
            index as f64
                * ratio;


        let left =
            position.floor() as usize;


        let fraction =
            (
                position
                    - left as f64
            ) as f32;


        if left + 1
            < audio.len()
        {

            let a =
                audio[left];


            let b =
                audio[left + 1];


            output.push(
                a + (b - a)
                    * fraction
            );

        } else if left
            < audio.len()
        {

            output.push(
                audio[left]
            );
        }
    }


    output
}


// ============================================================
// RMS
// ============================================================

fn calculate_rms(
    samples: &[f32],
) -> f32 {

    if samples.is_empty() {
        return 0.0;
    }


    let mut sum =
        0.0f64;


    for &sample in samples {

        let value =
            sample as f64;


        sum +=
            value * value;
    }


    (
        sum
            / samples.len() as f64
    )
        .sqrt() as f32
}


// ============================================================
// CONVERSION
// ============================================================

fn convert_i16_to_f32(
    data: &[i16],
) -> Vec<f32> {

    data.iter()
        .map(
            |&sample|
                sample as f32
                    / i16::MAX as f32
        )
        .collect()
}


fn convert_u16_to_f32(
    data: &[u16],
) -> Vec<f32> {

    data.iter()
        .map(
            |&sample| {

                (
                    sample as f32
                        - 32768.0
                )
                    / 32768.0
            }
        )
        .collect()
}