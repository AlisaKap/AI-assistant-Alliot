const WAKE_WORD_VARIANTS: &[&str] = &[
    // Нормальное распознавание
    "аллиот",

    // Частые варианты
    "алиот",
    "алиод",
    "аллиод",
    "аллит",
    "аллиут",

    // Whisper может менять начало / гласные
    "алет",
    "алед",
    "алюд",
    "алют",

    // Варианты через "э"
    "элиот",
    "элиод",
    "эллиот",
    "эллиод",
    "эллоот",
    "эльот",
    "эльотт",
    "эллюд",
    "эллиут",
    "элют",
    "эйлот",

    // Если Whisper разделил имя на несколько слов
    "али от",
    "алли от",
    "ал лиот",
    "а лиот",
    "эли от",
    "элли от",
    "эй лот",
    "эй лед",

    // Лишний звук в начале
    "наллиот",
];

/// Проверяет, содержит ли текст обращение к Alliot.
///
/// Примеры:
///
/// "аллиот открой блендер"
/// -> true
///
/// "эльот открой блендер"
/// -> true
///
/// "алюд открой блендер"
/// -> true
///
/// "открой блендер"
/// -> false
pub fn contains_wake_word(text: &str) -> bool {
    let normalized = normalize(text);

    if normalized.is_empty() {
        return false;
    }

    find_wake_word(&normalized).is_some()
}

/// Удаляет первое найденное обращение к Alliot.
///
/// Например:
///
/// "аллиот открой блендер"
/// -> "открой блендер"
///
/// "эльот открой blender"
/// -> "открой blender"
///
/// Если wake word не найден:
///
/// "открой блендер"
/// -> "открой блендер"
pub fn remove_wake_word(text: &str) -> String {
    let normalized = normalize(text);

    if normalized.is_empty() {
        return String::new();
    }

    let Some((start, end)) = find_wake_word(&normalized) else {
        return normalized;
    };

    let before = normalized[..start].trim();
    let after = normalized[end..].trim();

    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),

        (true, false) => after.to_string(),

        (false, true) => before.to_string(),

        (false, false) => {
            format!("{} {}", before, after)
        }
    }
}

/// Ищет первое вхождение wake word.
///
/// Возвращает диапазон байтов:
///
/// (start, end)
///
/// Диапазон используется для безопасного удаления
/// wake word из UTF-8 строки.
fn find_wake_word(
    text: &str,
) -> Option<(usize, usize)> {

    let words: Vec<&str> =
        text.split_whitespace().collect();

    if words.is_empty() {
        return None;
    }

    /*
     * Сначала проверяем длинные варианты.
     *
     * Например:
     *
     * "алли от"
     *
     * должен проверяться раньше,
     * чем отдельные части.
     */
    let mut variants =
        WAKE_WORD_VARIANTS.to_vec();

    variants.sort_by(|a, b| {
        let a_words =
            a.split_whitespace().count();

        let b_words =
            b.split_whitespace().count();

        b_words
            .cmp(&a_words)
            .then_with(|| {
                b.len().cmp(&a.len())
            })
    });

    /*
     * Проверяем каждый вариант
     * как последовательность отдельных слов.
     *
     * Это важно.
     *
     * Мы не используем:
     *
     * text.contains("алиот")
     *
     * потому что это может дать ложные совпадения
     * внутри другого слова.
     */
    for variant in variants {
        let variant_words: Vec<&str> =
            variant
                .split_whitespace()
                .collect();

        if variant_words.is_empty() {
            continue;
        }

        if variant_words.len() > words.len() {
            continue;
        }

        for index in 0..=
            words.len() - variant_words.len()
        {
            let window =
                &words[
                    index
                        ..index
                            + variant_words.len()
                ];

            if window != variant_words {
                continue;
            }

            /*
             * Нашли совпадение.
             *
             * Теперь вычисляем диапазон
             * непосредственно в исходной
             * нормализованной строке.
             */

            let start =
                if index == 0 {
                    0
                } else {
                    find_nth_word_start(
                        text,
                        index,
                    )?
                };

            let end_word_index =
                index
                    + variant_words.len()
                    - 1;

            let end =
                find_word_end(
                    text,
                    end_word_index,
                )?;

            return Some((start, end));
        }
    }

    None
}

/// Возвращает позицию начала N-го слова.
///
/// Например:
///
/// "аллиот открой blender"
///
/// word 0 -> начало "аллиот"
/// word 1 -> начало "открой"
/// word 2 -> начало "blender"
fn find_nth_word_start(
    text: &str,
    word_index: usize,
) -> Option<usize> {

    let mut current_word = 0;

    for (index, character) in text.char_indices() {

        if character.is_whitespace() {
            continue;
        }

        /*
         * Если это первый символ слова,
         * проверяем, нужное ли это слово.
         */
        let previous_is_whitespace =
            index == 0
                || text[..index]
                    .chars()
                    .last()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);

        if previous_is_whitespace {

            if current_word == word_index {
                return Some(index);
            }

            current_word += 1;
        }
    }

    None
}

/// Возвращает позицию конца N-го слова.
fn find_word_end(
    text: &str,
    word_index: usize,
) -> Option<usize> {

    let start =
        find_nth_word_start(
            text,
            word_index,
        )?;

    for (offset, character)
        in text[start..].char_indices()
    {
        if character.is_whitespace() {
            return Some(
                start + offset
            );
        }
    }

    Some(text.len())
}

/// Нормализует текст Whisper.
///
/// Пример:
///
/// "Аллиот, открой Blender!"
///
/// ->
///
/// "аллиот открой blender"
///
/// Делается:
///
/// 1. lowercase
/// 2. ё -> е
/// 3. удаление пунктуации
/// 4. нормализация пробелов
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace('ё', "е")
        .chars()
        .map(|character| {

            if character.is_alphanumeric()
                || character.is_whitespace()
            {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
