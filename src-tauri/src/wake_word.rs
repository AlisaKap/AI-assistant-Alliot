// ============================================================
// WAKE WORD
// ============================================================

const WAKE_WORD_VARIANTS: &[&str] = &[
    "аллиот",
    "алиот",
    "алиод",
    "аллиод",
    "аллит",
    "алет",
    "алед",
    "алло",

    "али от",
    "алли от",
    "ал лиот",
    "а лиот",

    "элиот",
    "элиод",
    "эллиот",
    "эллот",
    "эллиод",
    "элли от",
    "элли",
    "эли",
    "эло",
    "элло",
    "эллюд",
    "эллиут",
    "элют",
    "элет",
    "эллото",
    "эллут",
    "айлот",

    "наллиот",
    "налиот",
];


// ============================================================
// CONTAINS
// ============================================================

pub fn contains_wake_word(
    text: &str,
) -> bool {

    let normalized =
        normalize(text);

    WAKE_WORD_VARIANTS
        .iter()
        .any(|variant| {
            contains_variant(
                &normalized,
                variant,
            )
        })
}


// ============================================================
// EXTRACT COMMAND
// ============================================================
//
// Examples:
//
// "аллиот"
//     -> Some("")
//
// "аллиот открой blender"
//     -> Some("открой blender")
//
// "привет аллиот открой blender"
//     -> Some("открой blender")
//
// "открой blender"
//     -> None
//
// ============================================================

pub fn extract_command(
    text: &str,
) -> Option<String> {

    let normalized =
        normalize(text);


    let (start, end) =
        find_wake_word(
            &normalized
        )?;


    let _ = start;


    let command =
        normalized[end..]
            .trim()
            .to_string();


    Some(command)
}


// ============================================================
// REMOVE WAKE WORD
// ============================================================

pub fn remove_wake_word(
    text: &str,
) -> String {

    let normalized =
        normalize(text);


    let Some((_start, end)) =
        find_wake_word(
            &normalized
        )
    else {
        return normalized;
    };


    normalized[end..]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}


// ============================================================
// FIND WAKE WORD
// ============================================================

fn find_wake_word(
    text: &str,
) -> Option<(usize, usize)> {

    let normalized =
        normalize(text);


    let mut best_match:
        Option<(usize, usize)> =
        None;


    for variant in
        WAKE_WORD_VARIANTS
    {

        let mut search_from =
            0usize;


        while let Some(relative_start) =
            normalized[search_from..]
                .find(variant)
        {

            let start =
                search_from
                    + relative_start;


            let end =
                start
                    + variant.len();


            if is_word_boundary(
                &normalized,
                start,
                end,
            ) {

                match best_match {

                    None => {
                        best_match =
                            Some((start, end));
                    }

                    Some((
                        current_start,
                        current_end,
                    )) => {

                        let current_length =
                            current_end
                                - current_start;


                        let new_length =
                            end - start;


                        if start
                            < current_start
                            || (
                                start
                                    == current_start
                                && new_length
                                    > current_length
                            )
                        {
                            best_match =
                                Some((start, end));
                        }
                    }
                }

                break;
            }


            search_from =
                end;
        }
    }


    best_match
}


// ============================================================
// VARIANT MATCH
// ============================================================

fn contains_variant(
    text: &str,
    variant: &str,
) -> bool {

    let mut search_from =
        0usize;


    while let Some(relative_start) =
        text[search_from..]
            .find(variant)
    {

        let start =
            search_from
                + relative_start;


        let end =
            start
                + variant.len();


        if is_word_boundary(
            text,
            start,
            end,
        ) {
            return true;
        }


        search_from =
            end;
    }


    false
}


// ============================================================
// WORD BOUNDARY
// ============================================================

fn is_word_boundary(
    text: &str,
    start: usize,
    end: usize,
) -> bool {

    let before_ok =
        if start == 0 {

            true

        } else {

            text[..start]
                .chars()
                .next_back()
                .map(|character| {
                    !character.is_alphanumeric()
                })
                .unwrap_or(true)
        };


    let after_ok =
        if end >= text.len() {

            true

        } else {

            text[end..]
                .chars()
                .next()
                .map(|character| {
                    !character.is_alphanumeric()
                })
                .unwrap_or(true)
        };


    before_ok
        && after_ok
}


// ============================================================
// NORMALIZE
// ============================================================

fn normalize(
    text: &str,
) -> String {

    text
        .to_lowercase()
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