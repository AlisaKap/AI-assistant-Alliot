use std::process::Command;

/// Открывает обычное приложение.
fn launch(path: &str) -> Result<(), String> {
    Command::new(path)
        .spawn()
        .map_err(|error| format!("Не удалось запустить {}: {}", path, error))?;

    Ok(())
}

/// Открывает приложение с запросом прав администратора через UAC.
fn launch_as_admin(path: &str) -> Result<(), String> {
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                path
            ),
        ])
        .spawn()
        .map_err(|error| {
            format!(
                "Не удалось запустить {} от имени администратора: {}",
                path, error
            )
        })?;

    Ok(())
}

/// Открывает приложение по системной команде.
pub fn open_application(application: &str) -> Result<(), String> {
    match application {
        "browser" => {
            Command::new("explorer.exe")
                .arg("https://www.google.com")
                .spawn()
                .map_err(|error| format!("Не удалось открыть браузер: {}", error))?;

            Ok(())
        }

        "photoshop" => {
            launch(
                r"C:\Program Files\Adobe\Adobe Photoshop CC 2019\Photoshop.exe",
            )
        }

        "illustrator" => {
            launch(
                r"C:\Program Files\Adobe\Adobe Illustrator 2024\Support Files\Contents\Windows\Illustrator.exe",
            )
        }

        "blender" => {
            launch(
                r"C:\Program Files\Blender Foundation\Blender 5.2\blender-launcher.exe",
            )
        }

        "spine" => {
            launch(
                r"E:\Downloads\Spine Pro v3.8.75 (WIN)\Setup\Spine.exe",
            )
        }

        "discord" => {
            launch(
                r"C:\Users\alisa\AppData\Local\Discord\app-1.0.9255\Discord.exe",
            )
        }

        "figma" => {
            launch(
                r"C:\Users\alisa\AppData\Local\Figma\app-126.8.16\Figma.exe",
            )
        }

        "webstorm" => {
            launch(
                r"C:\Program Files\JetBrains\WebStorm 2024.3.2.1\bin\webstorm64.exe",
            )
        }

        _ => {
            Err(format!("Неизвестное приложение: {}", application))
        }
    }
}

/// Открывает рабочую среду.
pub fn open_workspace() -> Result<(), String> {
    launch(
        r"E:\Desktop\TgWsProxy_windows.exe",
    )?;

    launch_as_admin(
        r"E:\Desktop\zapret-discord-youtube-1.9.8b\general (ALT).bat",
    )?;

    Ok(())
}