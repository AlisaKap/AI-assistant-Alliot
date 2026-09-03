use crate::assistant::AssistantAction;

/// Намерение пользователя.
#[derive(Debug, Clone, Copy)]
enum CommandIntent {
    Respond,
    Open,
    Workspace,
}

/// Приложения, которые умеет открывать Alliot.
#[derive(Debug, Clone, Copy)]
enum Application {
    Browser,
    Photoshop,
    Illustrator,
    Blender,
    Spine,
    Discord,
    Figma,
    WebStorm,
}

impl Application {
    /// Внутреннее имя приложения.
    fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Photoshop => "photoshop",
            Self::Illustrator => "illustrator",
            Self::Blender => "blender",
            Self::Spine => "spine",
            Self::Discord => "discord",
            Self::Figma => "figma",
            Self::WebStorm => "webstorm",
        }
    }
}

/// Приводит текст Whisper к единому виду.
fn normalize_text(text: &str) -> String {
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

/// Проверяет наличие одного из вариантов в тексте.
fn contains_any(text: &str, variants: &[&str]) -> bool {
    variants
        .iter()
        .any(|variant| text.contains(variant))
}

/// Определяет намерение пользователя.
fn extract_intent(text: &str) -> Option<CommandIntent> {
    // Открытие / запуск приложения.
    // Аналогично для "запустить" и "включить".
    if contains_any(
        text,
        &[
            "откр",
            "отскр",
            "запуст",
            "включ",
            "ключ",
        ],
    ) {
        return Some(CommandIntent::Open);
    }

    // Рабочее пространство.
    if contains_any(
        text,
        &[
            "поднять щиты",
            "подними щиты",
            "поднимай щиты",
            "поднять щит",
            "подними щит",
            "щиты поднять",
        ],
    ) {
        return Some(CommandIntent::Workspace);
    }

    // Приветствие.
    if contains_any(
        text,
        &[
            "привет",
            "ку",
            "hello",
            "хелло",
        ],
    ) {
        return Some(CommandIntent::Respond);
    }

    // Прощание.
    if contains_any(
        text,
        &[
            "спокойной ночи",
            "давай пока",
            "пока",
            "bye",
            "goodbye",
        ],
    ) {
        return Some(CommandIntent::Respond);
    }

    None
}

/// Определяет приложение из текста.
fn extract_application(text: &str) -> Option<Application> {
    // Photoshop
    if contains_any(
        text,
        &[
            "фотошоп",
            "фотош",
            "фото шоп",
            "photoshop",
            "photosho",
        ],
    ) {
        return Some(Application::Photoshop);
    }

    // Illustrator
    if contains_any(
        text,
        &[
            "иллюстратор",
            "илюстратор",
            "иллюстрат",
            "илюстрат",
            "illustrator",
            "illustrato",
        ],
    ) {
        return Some(Application::Illustrator);
    }

    // Blender
    if contains_any(
        text,
        &[
            "блендер",
            "бленд",
            "блэндер",
            "blender",
        ],
    ) {
        return Some(Application::Blender);
    }

    // Spine
    if contains_any(
        text,
        &[
            "спайн",
            "спай",
            "spine",
        ],
    ) {
        return Some(Application::Spine);
    }

    // Discord
    if contains_any(
        text,
        &[
            "дискорд",
            "дискор",
            "discord",
        ],
    ) {
        return Some(Application::Discord);
    }

    // Figma
    if contains_any(
        text,
        &[
            "фигма",
            "фигм",
            "figma",
        ],
    ) {
        return Some(Application::Figma);
    }

    // WebStorm
    if contains_any(
        text,
        &[
            "вебшторм",
            "веб штор",
            "вебштор",
            "webstorm",
            "web storm",
        ],
    ) {
        return Some(Application::WebStorm);
    }

    // Browser
    if contains_any(
        text,
        &[
            "браузер",
            "брауз",
            "browser",
            "chrome",
            "хром",
        ],
    ) {
        return Some(Application::Browser);
    }

    None
}

/// Формирует ответ на текстовую команду.
fn build_response(text: &str) -> AssistantAction {
    if contains_any(
        text,
        &[
            "привет",
            "ку",
            "hello",
            "хелло",
        ],
    ) {
        return AssistantAction::Respond(
            "Привет. Я Alliot.".to_string(),
        );
    }

    if contains_any(
        text,
        &[
            "спокойной ночи",
            "давай пока",
            "пока",
            "bye",
            "goodbye",
        ],
    ) {
        return AssistantAction::Respond(
            "До встречи.".to_string(),
        );
    }

    AssistantAction::Unknown(text.to_string())
}

/// Удаляет wake word из начала команды.
pub fn remove_wake_word(text: &str) -> String {
    let normalized =
        normalize_text(text);

    let wake_words = [
        "аллиот",
        "алиот",
        "аллиод",
        "алиод",
        "алет",
        "алед",
        "элиот",
        "эллиот",
    ];

    let words =
        normalized
            .split_whitespace()
            .collect::<Vec<_>>();

    if words.is_empty() {
        return String::new();
    }

    if wake_words.contains(&words[0]) {
        return words[1..].join(" ");
    }

    normalized
}

/// Главный парсер команды.
pub fn parse_command(text: &str) -> AssistantAction {
    let text = remove_wake_word(text);

    println!(
        "Парсер получил: {:?}",
        text
    );

    if text.is_empty() {
        return AssistantAction::Unknown(
            String::new()
        );
    }

    let intent =
        extract_intent(&text);

    println!(
        "Намерение: {:?}",
        intent
    );

    match intent {

        Some(CommandIntent::Open) => {

            let application =
                extract_application(&text);

            println!(
                "Приложение: {:?}",
                application
            );

            match application {

                Some(application) => {
                    AssistantAction::OpenApplication(
                        application
                            .as_str()
                            .to_string(),
                    )
                }

                None => {
                    AssistantAction::Unknown(
                        text,
                    )
                }
            }
        }

        Some(CommandIntent::Workspace) => {
            AssistantAction::OpenWorkspace
        }

        Some(CommandIntent::Respond) => {
            build_response(&text)
        }

        None => {

            println!(
                "Намерение не распознано"
            );

            AssistantAction::Unknown(
                text
            )
        }
    }
}