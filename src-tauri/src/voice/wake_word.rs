const WAKE_WORD_VARIANTS: &[&str] = &[
    "аллиот",
    "алиот",
    "алиод",
    "аллиод",
    "аллит",
    "алет",
    "алед",
    "али от",
    "алли от",
    "ал лиот",
    "а лиот",
    "элли от",
    "эллюд",
    "эллиут",
    "элют",
    "наллиот",
    "элиот",
    "элли от",
];

pub fn contains_wake_word(text: &str) -> bool {
    let text = normalize(text);

    WAKE_WORD_VARIANTS
        .iter()
        .any(|variant| text.contains(variant))
}

pub fn remove_wake_word(text: &str) -> String {
    let mut text = normalize(text);

    for variant in WAKE_WORD_VARIANTS {
        text = text.replace(variant, " ");
    }

    text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace('ё', "ё")
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