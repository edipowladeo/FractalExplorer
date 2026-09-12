use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub debug: bool,
    pub renderer: RendererConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RendererConfig {
    pub width: usize,
    pub height: usize,
    pub max_iterations: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug: false,
            renderer: RendererConfig::default(),
        }
    }
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            max_iterations: 256,
        }
    }
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn loads_window_size_and_debug_from_toml() {
        let config: AppConfig = toml::from_str(
            r#"
            debug = true

            [renderer]
            width = 800
            height = 600
            "#,
        )
        .unwrap();

        assert!(config.debug);
        assert_eq!(config.renderer.width, 800);
        assert_eq!(config.renderer.height, 600);
        assert_eq!(config.renderer.max_iterations, 256);
    }
}
