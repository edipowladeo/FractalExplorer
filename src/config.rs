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
    pub orchestrator: OrchestratorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OrchestratorConfig {
    pub tile: TileConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TileConfig {
    pub width: u32,
    pub height: u32,
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
    pub starting_point: String,
    pub rendering_method: String,
    pub perturbation_fallback: bool,
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
            orchestrator: OrchestratorConfig::default(),
        }
    }
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            tile: TileConfig::default(),
        }
    }
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
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
            palette: Palette::Rainbow,
            palette_period: 5.0,
            starting_point: "x=0.0, y=0.0".to_string(),
            rendering_method: "f64".to_string(),
            perturbation_fallback: false,
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
    pub fn starting_point_coordinates(&self) -> Result<crate::geometry::ComplexPoint<f64>, String> {
        let (x_text, y_text) = self.starting_point.split_once(',').ok_or_else(|| {
            "starting_point deve usar o formato 'x=<valor>, y=<valor>'".to_string()
        })?;
        let x = x_text
            .trim()
            .strip_prefix("x=")
            .ok_or_else(|| "starting_point deve iniciar com 'x='".to_string())?
            .parse::<f64>()
            .map_err(|_| "valor x inválido em starting_point".to_string())?;
        let y = y_text
            .trim()
            .strip_prefix("y=")
            .ok_or_else(|| "starting_point deve conter 'y='".to_string())?
            .parse::<f64>()
            .map_err(|_| "valor y inválido em starting_point".to_string())?;
        Ok(crate::geometry::ComplexPoint::new(x, y))
    }

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
            starting_point = "x=-0.743643887037151, y=0.131825904205330"
            perturbation_fallback = true

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
        assert_eq!(config.renderer.rendering_method, "f64");
        assert!(config.renderer.perturbation_fallback);
        assert_eq!(config.orchestrator.tile.width, 800);
        assert_eq!(config.orchestrator.tile.height, 600);
        assert_eq!(
            config.renderer.starting_point_coordinates().unwrap(),
            crate::geometry::ComplexPoint::new(-0.743643887037151, 0.131825904205330)
        );
    }

    #[test]
    fn perturbation_fallback_is_disabled_by_default() {
        assert!(!AppConfig::default().renderer.perturbation_fallback);
    }
}
