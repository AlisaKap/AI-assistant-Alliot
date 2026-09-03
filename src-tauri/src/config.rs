/// Application configuration.
pub struct AppConfig {
    /// Application name.
    pub name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "Alliot".to_string(),
        }
    }
}