use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use whisper_rs::{
    FullParams,
    SamplingStrategy,
    WhisperContext,
    WhisperContextParameters,
};

const MODEL_PATH: &str = "models/ggml-small.bin";

/// Whisper загружается один раз и остаётся в памяти
/// до завершения приложения.
static WHISPER_CONTEXT: OnceLock<Result<WhisperContext, String>> =
    OnceLock::new();

/// Возвращает глобальный контекст Whisper.
///
/// Модель физически загружается только при первом вызове.
fn get_whisper_context() -> Result<&'static WhisperContext, String> {

    whisper_rs::print_system_info();

    let context = WHISPER_CONTEXT.get_or_init(|| {
        println!("Загрузка модели Whisper...");

        let mut params = WhisperContextParameters::default();

        params.use_gpu = true;
        params.gpu_device = 0;

        WhisperContext::new_with_params(
            MODEL_PATH,
            params,
        )
        .map_err(|error| {
            format!("Не удалось загрузить модель Whisper: {error}")
        })
    });

    context.as_ref().map_err(|error| error.clone())
}

pub fn listen() -> Result<String, String> {
    let audio = record_audio()?;

    if audio.is_empty() {
        return Err("Аудио не записано".to_string());
    }

    let audio_16k = resample_to_16khz(
        &audio,
        48_000,
    );

    if audio_16k.is_empty() {
        return Err(
            "После обработки аудио оказалось пустым".to_string()
        );
    }

    let text = transcribe(&audio_16k)?;

    Ok(normalize_voice_text(&text))
}

/// Записывает голос с микрофона.
///
/// Запись:
/// - начинается сразу;
/// - ждёт появления речи;
/// - после речи ждёт паузу;
/// - максимум длится 5 секунд.
fn record_audio() -> Result<Vec<f32>, String> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .ok_or_else(|| {
            "Микрофон не найден".to_string()
        })?;

    let supported_config = device
        .default_input_config()
        .map_err(|error| {
            format!(
                "Не удалось получить настройки микрофона: {error}"
            )
        })?;

    let sample_format =
        supported_config.sample_format();

    let config: cpal::StreamConfig =
        supported_config.into();

    let sample_rate =
        config.sample_rate.0;

    if config.channels != 1 {
        return Err(format!(
            "Ожидался mono-микрофон, получено {} каналов",
            config.channels
        ));
    }

    let samples: Arc<Mutex<Vec<f32>>> =
        Arc::new(Mutex::new(Vec::new()));

    let samples_clone =
        Arc::clone(&samples);

    let err_fn = |error| {
        eprintln!(
            "Ошибка микрофона: {error}"
        );
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        if let Ok(mut buffer) =
                            samples_clone.lock()
                        {
                            buffer.extend_from_slice(data);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|error| {
                    format!(
                        "Не удалось запустить микрофон: {error}"
                    )
                })?
        }

        cpal::SampleFormat::I16 => {
            let samples_clone =
                Arc::clone(&samples);

            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        if let Ok(mut buffer) =
                            samples_clone.lock()
                        {
                            for &sample in data {
                                buffer.push(
                                    sample as f32
                                        / i16::MAX as f32,
                                );
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|error| {
                    format!(
                        "Не удалось запустить микрофон: {error}"
                    )
                })?
        }

        cpal::SampleFormat::U16 => {
            let samples_clone =
                Arc::clone(&samples);

            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        if let Ok(mut buffer) =
                            samples_clone.lock()
                        {
                            for &sample in data {
                                buffer.push(
                                    (sample as f32
                                        - 32768.0)
                                        / 32768.0,
                                );
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|error| {
                    format!(
                        "Не удалось запустить микрофон: {error}"
                    )
                })?
        }

        format => {
            return Err(format!(
                "Неподдерживаемый формат микрофона: {format:?}"
            ));
        }
    };

    stream
        .play()
        .map_err(|error| {
            format!(
                "Не удалось начать запись: {error}"
            )
        })?;

    // Максимальная длительность одной команды.
    const MAX_RECORDING_SECONDS: f32 = 5.0;

    // Минимальная длительность речи.
    const MIN_SPEECH_SECONDS: f32 = 0.25;

    // Пауза после речи, после которой заканчиваем запись.
    const SILENCE_SECONDS: f32 = 0.8;

    // Порог громкости для определения речи.
    const VOLUME_THRESHOLD: f32 = 0.008;

    let max_samples =
        (sample_rate as f32
            * MAX_RECORDING_SECONDS) as usize;

    let silence_duration =
        Duration::from_secs_f32(
            SILENCE_SECONDS,
        );

    // Анализируем последние 50 ms аудио.
    let analysis_window =
        (sample_rate as f32 * 0.05) as usize;

    let mut speech_detected = false;

    let mut speech_start: Option<Instant> =
        None;

    let mut last_voice_time: Option<Instant> =
        None;

    loop {
        std::thread::sleep(
            Duration::from_millis(50),
        );

        let current_len = {
            let buffer = samples
                .lock()
                .map_err(|_| {
                    "Не удалось получить записанный звук"
                        .to_string()
                })?;

            buffer.len()
        };

        // Абсолютный максимум записи.
        if current_len >= max_samples {
            break;
        }

        // Недостаточно данных для анализа.
        if current_len < analysis_window {
            continue;
        }

        let volume = {
            let buffer = samples
                .lock()
                .map_err(|_| {
                    "Не удалось получить записанный звук"
                        .to_string()
                })?;

            let start =
                current_len.saturating_sub(
                    analysis_window,
                );

            let recent =
                &buffer[start..current_len];

            if recent.is_empty() {
                0.0
            } else {
                // RMS громкости.
                let sum = recent
                    .iter()
                    .map(|sample| sample * sample)
                    .sum::<f32>();

                (sum / recent.len() as f32).sqrt()
            }
        };

        let now = Instant::now();

        if volume > VOLUME_THRESHOLD {
            // Начало речи.
            if !speech_detected {
                speech_detected = true;
                speech_start = Some(now);
            }

            last_voice_time = Some(now);
        } else if speech_detected {
            // Проверяем длительность паузы.
            if let Some(last_voice) =
                last_voice_time
            {
                if now.duration_since(last_voice)
                    >= silence_duration
                {
                    let speech_duration =
                        speech_start
                            .map(|start| {
                                now.duration_since(start)
                            })
                            .unwrap_or_default();

                    if speech_duration
                        .as_secs_f32()
                        >= MIN_SPEECH_SECONDS
                    {
                        break;
                    }
                }
            }
        }
    }

    drop(stream);

    let samples = samples
        .lock()
        .map_err(|_| {
            "Не удалось получить записанный звук"
                .to_string()
        })?
        .clone();

    if samples.is_empty() {
        return Err(
            "Микрофон не записал ни одного сэмпла"
                .to_string()
        );
    }

    if !speech_detected {
        return Err(
            "Речь не обнаружена".to_string()
        );
    }

    Ok(samples)
}

/// Ресемплинг аудио в 16 kHz.
///
/// Whisper работает с mono PCM 16 kHz.
/// Используем линейную интерполяцию.
fn resample_to_16khz(
    audio: &[f32],
    input_sample_rate: u32,
) -> Vec<f32> {
    const OUTPUT_RATE: u32 = 16_000;

    if input_sample_rate == OUTPUT_RATE {
        return audio.to_vec();
    }

    if audio.is_empty() {
        return Vec::new();
    }

    let ratio =
        input_sample_rate as f64
            / OUTPUT_RATE as f64;

    let output_len =
        (audio.len() as f64 / ratio)
            .floor() as usize;

    let mut output =
        Vec::with_capacity(output_len);

    for i in 0..output_len {
        let position =
            i as f64 * ratio;

        let index =
            position.floor() as usize;

        let fraction =
            (position - index as f64)
                as f32;

        if index + 1 < audio.len() {
            let a = audio[index];
            let b = audio[index + 1];

            output.push(
                a + (b - a) * fraction
            );
        } else if index < audio.len() {
            output.push(audio[index]);
        }
    }

    output
}

/// Распознаёт аудио через Whisper.
fn transcribe(
    audio: &[f32],
) -> Result<String, String> {
    let ctx =
        get_whisper_context()?;

    let mut state = ctx
        .create_state()
        .map_err(|error| {
            format!(
                "Не удалось создать состояние Whisper: {error}"
            )
        })?;

    let mut params =
        FullParams::new(
            SamplingStrategy::Greedy {
                best_of: 1,
            },
        );

    params.set_language(Some("ru"));
    params.set_translate(false);

    // Отключаем внутренний вывод Whisper.
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // Каждая команда распознаётся независимо.
    params.set_no_context(true);

    // Отсекаем участки, которые Whisper считает тишиной.
    params.set_no_speech_thold(0.6);

    state
        .full(params, audio)
        .map_err(|error| {
            format!(
                "Ошибка распознавания Whisper: {error}"
            )
        })?;

    let num_segments =
        state.full_n_segments();

    let mut result =
        String::new();

    for index in 0..num_segments {
        if let Some(segment) =
            state.get_segment(index)
        {
            let text = segment
                .to_str()
                .map_err(|error| {
                    error.to_string()
                })?;

            result.push_str(text);
            result.push(' ');
        }
    }

    let result =
        result.trim().to_string();

    if result.is_empty() {
        return Err(
            "Whisper не распознал речь"
                .to_string()
        );
    }

    Ok(result)
}

/// Исправляет распространённые ошибки Whisper
/// при распознавании имени Alliot.
fn normalize_voice_text(
    text: &str,
) -> String {
    let replacements = [
        ("алёд", "аллиот"),
        ("а, лёд", "аллиот"),
        ("алёт", "аллиот"),
        ("алед", "аллиот"),
        ("алет", "аллиот"),
        ("алиот", "аллиот"),
        ("алиод", "аллиот"),
        ("аллиод", "аллиот"),
        ("аллит", "аллиот"),
        ("аллиотт", "аллиот"),
        ("али от", "аллиот"),
        ("алли от", "аллиот"),
        ("али ёт", "аллиот"),
        ("ал лиот", "аллиот"),
        ("а лиот", "аллиот"),
    ];

    let mut result =
        text.to_lowercase();

    for (wrong, correct) in replacements {
        result =
            result.replace(wrong, correct);
    }

    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}