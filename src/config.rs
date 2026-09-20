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
    pub workers: usize,
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
    pub deallocation_ratio: f64,
    pub max_apparent_pixel_size_exponent: i32,
    pub min_apparent_pixel_size: f64,
    pub zoom_multiplier: f64,
    pub palette: Palette,
    pub palette_period: f64,
    pub starting_point: String,
    pub rendering_method: String,
    pub precision_technique: String,
    pub precision_level: usize,
    pub perturbation_fallback: bool,
    pub debug: RendererDebugConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RendererDebugConfig {
    pub reduced_viewport: bool,
    pub reduced_viewport_allocation_ratio: f64,
    pub show_allocation_envelope: bool,
    pub text_overlay_global: bool,
    pub text_overlay_workers: bool,
    pub text_overlay_layers: bool,
    pub text_overlay_queue: bool,
    pub middle_click_coordinate_report: bool,
    pub layer_creation_diagnostics: bool,
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
            workers: 8,
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
            deallocation_ratio: 0.8,
            max_apparent_pixel_size_exponent: 3,
            min_apparent_pixel_size: 0.8,
            zoom_multiplier: 1.1,
            palette: Palette::Rainbow,
            palette_period: 5.0,
            starting_point: "x=0.0, y=0.0".to_string(),
            rendering_method: "f64".to_string(),
            precision_technique: "float".to_string(),
            precision_level: 1,
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
            text_overlay_global: true,
            text_overlay_workers: true,
            text_overlay_layers: true,
            text_overlay_queue: true,
            middle_click_coordinate_report: false,
            layer_creation_diagnostics: true,
        }
    }
}

impl RendererConfig {
    pub fn effective_max_iterations(&self) -> u32 {
        self.max_iterations
            .saturating_mul(self.precision_level as u32)
    }

    pub fn max_apparent_pixel_size(&self) -> f64 {
        2.0_f64.powi(self.max_apparent_pixel_size_exponent)
    }

    pub fn starting_point_coordinates(&self) -> Result<crate::geometry::ComplexPoint<f64>, String> {
        if self.starting_point.contains("zoom:") {
            return self.starting_view().map(|(point, _)| point);
        }
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

    pub fn starting_view(&self) -> Result<(crate::geometry::ComplexPoint<f64>, f64), String> {
        if !self.starting_point.contains("zoom:") {
            return self
                .starting_point_coordinates()
                .map(|point| (point, self.max_apparent_pixel_size()));
        }
        let mut fields = self.starting_point.split_whitespace();
        let parse = |label: &str, fields: &mut std::str::SplitWhitespace<'_>| {
            if fields.next() != Some(label) {
                return Err(format!("starting_point deve conter '{label} <valor>'"));
            }
            fields
                .next()
                .ok_or_else(|| format!("valor ausente após '{label}' em starting_point"))?
                .parse::<f64>()
                .map_err(|_| format!("valor inválido após '{label}' em starting_point"))
        };
        let x = parse("x:", &mut fields)?;
        let y = parse("y:", &mut fields)?;
        let zoom_exponent = parse("zoom:", &mut fields)?;
        if fields.next().is_some() {
            return Err("starting_point contém dados adicionais".to_string());
        }
        let zoom = 2.0_f64.powf(zoom_exponent);
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err("expoente de zoom inválido em starting_point".to_string());
        }
        Ok((crate::geometry::ComplexPoint::new(x, y), zoom))
    }

    pub fn effective_allocation_ratio(&self) -> f64 {
        if self.debug.reduced_viewport {
            self.debug.reduced_viewport_allocation_ratio
        } else {
            self.allocation_ratio
        }
    }

    pub fn effective_deallocation_ratio(&self) -> f64 {
        self.deallocation_ratio
            .max(self.effective_allocation_ratio())
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
    use super::{AppConfig, RendererConfig};

    #[test]
    fn loads_window_size_and_debug_from_toml() {
        let config: AppConfig = toml::from_str(
            r#"
            debug_global = true

            [renderer]
            width = 800
            height = 600
            allocation_ratio = 1.2
            deallocation_ratio = 0.8
            max_apparent_pixel_size_exponent = 3
            min_apparent_pixel_size = 0.8
            zoom_multiplier = 1.1
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
        assert_eq!(config.renderer.deallocation_ratio, 0.8);
        assert_eq!(config.renderer.max_apparent_pixel_size_exponent, 3);
        assert_eq!(config.renderer.max_apparent_pixel_size(), 8.0);
        assert_eq!(config.renderer.min_apparent_pixel_size, 0.8);
        assert_eq!(config.renderer.effective_deallocation_ratio(), 0.8);
        assert_eq!(config.renderer.zoom_multiplier, 1.1);
        assert_eq!(config.orchestrator.tile.width, 800);
        assert_eq!(config.orchestrator.tile.height, 600);
        assert_eq!(
            config.renderer.starting_point_coordinates().unwrap(),
            crate::geometry::ComplexPoint::new(-0.743643887037151, 0.131825904205330)
        );
    }

    #[test]
    fn accepts_coordinate_starting_point_without_zoom() {
        let mut config = RendererConfig::default();
        config.starting_point = "x=-1.50, y=0.0".to_string();

        assert_eq!(
            config.starting_view().unwrap(),
            (crate::geometry::ComplexPoint::new(-1.5, 0.0), 8.0)
        );
    }

    #[test]
    fn perturbation_fallback_is_disabled_by_default() {
        assert!(!AppConfig::default().renderer.perturbation_fallback);
    }

    #[test]
    fn layer_creation_diagnostics_are_enabled_by_default() {
        assert!(
            AppConfig::default()
                .renderer
                .debug
                .layer_creation_diagnostics
        );
    }

    #[test]
    fn deallocation_ratio_silently_uses_allocation_ratio_when_smaller() {
        let config: super::RendererConfig =
            toml::from_str("allocation_ratio = 1.2\ndeallocation_ratio = 0.5").unwrap();

        assert_eq!(config.effective_deallocation_ratio(), 1.2);
    }

    #[test]
    fn uses_default_apparent_pixel_size_limits() {
        let config = super::RendererConfig::default();

        assert_eq!(config.max_apparent_pixel_size_exponent, 3);
        assert_eq!(config.max_apparent_pixel_size(), 8.0);
        assert_eq!(config.min_apparent_pixel_size, 0.8);
    }
}
