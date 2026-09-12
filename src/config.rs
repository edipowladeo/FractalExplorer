use crate::renderer::Palette;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub debug_global: bool,
    pub renderer: RendererConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RendererConfig {
    pub width: usize,
    pub height: usize,
    pub max_iterations: u32,
    pub allocation_ratio: f64,
    pub palette: Palette,
    pub palette_period: f64,
    pub debug: RendererDebugConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RendererDebugConfig {
    pub reduced_viewport: bool,
    pub reduced_viewport_allocation_ratio: f64,
    pub show_allocation_envelope: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug_global: false,
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
            allocation_ratio: 1.2,
            palette: Palette::Shade,
            palette_period: 5.0,
            debug: RendererDebugConfig::default(),
        }
    }
}

impl Default for RendererDebugConfig {
    fn default() -> Self {
        Self {
            reduced_viewport: false,
            reduced_viewport_allocation_ratio: 0.5,
            show_allocation_envelope: false,
        }
    }
}

impl RendererConfig {
    pub fn effective_allocation_ratio(&self) -> f64 {
        if self.debug.reduced_viewport {
            self.debug.reduced_viewport_allocation_ratio
        } else {
            self.allocation_ratio
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
            debug_global = true

            [renderer]
            width = 800
            height = 600
            allocation_ratio = 1.2
            palette = "rainbow"
            palette_period = 5.0

            [renderer.debug]
            reduced_viewport = true
            reduced_viewport_allocation_ratio = 0.5
            show_allocation_envelope = true
            "#,
        )
        .unwrap();

        assert!(config.debug_global);
        assert_eq!(config.renderer.width, 800);
        assert_eq!(config.renderer.height, 600);
        assert_eq!(config.renderer.max_iterations, 256);
        assert!(config.renderer.debug.reduced_viewport);
        assert_eq!(config.renderer.debug.reduced_viewport_allocation_ratio, 0.5);
        assert!(config.renderer.debug.show_allocation_envelope);
        assert_eq!(config.renderer.effective_allocation_ratio(), 0.5);
        assert_eq!(config.renderer.palette, crate::renderer::Palette::Rainbow);
        assert_eq!(config.renderer.palette_period, 5.0);
    }
}
