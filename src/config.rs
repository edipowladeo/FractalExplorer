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
    #[serde(deserialize_with = "deserialize_positive_u32")]
    pub precision: u32,
    pub allocation_ratio: f64,
    pub deallocation_ratio: f64,
    pub max_apparent_pixel_size_exponent: i32,
    pub min_apparent_pixel_size: f64,
    pub zoom_multiplier: f64,
    pub palette: Palette,
    pub palette_period: f64,
    pub starting_point: String,
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
            precision: 1,
            allocation_ratio: 1.2,
            deallocation_ratio: 0.8,
            max_apparent_pixel_size_exponent: 3,
            min_apparent_pixel_size: 0.8,
            zoom_multiplier: 1.1,
            palette: Palette::Rainbow,
            palette_period: 5.0,
            starting_point: "x: 0.000000000000000   y: 0.000000000000000   zoom: 3.000000000000000"
                .to_string(),
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
        }
    }
}

impl RendererConfig {
    pub fn max_apparent_pixel_size(&self) -> f64 {
        2.0_f64.powi(self.max_apparent_pixel_size_exponent)
    }

    pub fn starting_view(&self) -> Result<(crate::geometry::ComplexPoint<f64>, f64), String> {
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

fn deserialize_positive_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = u32::deserialize(deserializer)?;
    if value == 0 {
        return Err(serde::de::Error::custom(
            "precision deve ser um inteiro positivo",
        ));
    }
    Ok(value)
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
    fn uses_eight_workers_by_default() {
        assert_eq!(AppConfig::default().orchestrator.workers, 8);
    }

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
            starting_point = "x: -0.743643887037151   y: 0.131825904205330   zoom: 1.321928094887362"

            [renderer.debug]
            reduced_viewport = true
            reduced_viewport_allocation_ratio = 0.5
            show_allocation_envelope = true
            text_overlay_global = false
            text_overlay_workers = false
            text_overlay_layers = false
            text_overlay_queue = false
            "#,
        )
        .unwrap();

        assert!(config.debug_global);
        assert_eq!(config.renderer.width, 800);
        assert_eq!(config.renderer.height, 600);
        assert_eq!(config.renderer.max_iterations, 256);
        assert_eq!(config.renderer.precision, 1);
        assert!(config.renderer.debug.reduced_viewport);
        assert_eq!(config.renderer.debug.reduced_viewport_allocation_ratio, 0.5);
        assert!(config.renderer.debug.show_allocation_envelope);
        assert!(!config.renderer.debug.text_overlay_global);
        assert!(!config.renderer.debug.text_overlay_workers);
        assert!(!config.renderer.debug.text_overlay_layers);
        assert!(!config.renderer.debug.text_overlay_queue);
        assert_eq!(config.renderer.effective_allocation_ratio(), 0.5);
        assert_eq!(config.renderer.palette, crate::renderer::Palette::Rainbow);
        assert_eq!(config.renderer.palette_period, 5.0);
        assert_eq!(config.renderer.deallocation_ratio, 0.8);
        assert_eq!(config.renderer.max_apparent_pixel_size_exponent, 3);
        assert_eq!(config.renderer.max_apparent_pixel_size(), 8.0);
        assert_eq!(config.renderer.min_apparent_pixel_size, 0.8);
        assert_eq!(config.renderer.effective_deallocation_ratio(), 0.8);
        assert_eq!(config.renderer.zoom_multiplier, 1.1);
        assert_eq!(config.orchestrator.tile.width, 800);
        assert_eq!(config.orchestrator.tile.height, 600);
        let (point, zoom) = config.renderer.starting_view().unwrap();
        assert_eq!(
            point,
            crate::geometry::ComplexPoint::new(-0.743643887037151, 0.131825904205330)
        );
        assert!((zoom - 2.5).abs() < 1e-12);
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

    #[test]
    fn rejects_non_positive_renderer_precision() {
        for value in ["0", "-1"] {
            let result: Result<super::RendererConfig, _> =
                toml::from_str(&format!("precision = {value}"));
            assert!(result.is_err(), "precision {value} should be rejected");
        }
    }

    #[test]
    fn accepts_any_positive_renderer_precision() {
        let config: super::RendererConfig = toml::from_str("precision = 2147483647").unwrap();

        assert_eq!(config.precision, 2_147_483_647);
    }

    #[test]
    fn disables_middle_click_coordinate_reports_by_default_without_disabling_global_overlay() {
        let debug = super::RendererDebugConfig::default();

        assert!(!debug.middle_click_coordinate_report);
        assert!(debug.text_overlay_global);
    }

    #[test]
    fn loads_the_middle_click_coordinate_report_flag_from_toml() {
        let config: super::RendererConfig =
            toml::from_str("[debug]\nmiddle_click_coordinate_report = true").unwrap();

        assert!(config.debug.middle_click_coordinate_report);
    }

    #[test]
    fn loads_starting_point_and_zoom_from_the_copied_coordinate_format() {
        let config: super::RendererConfig = toml::from_str(
            "starting_point = \"x: -0.743643887037151   y: 0.131825904205330   zoom: 1.321928094887362\"",
        )
        .unwrap();

        let (point, zoom) = config.starting_view().unwrap();
        assert_eq!(
            point,
            crate::geometry::ComplexPoint::new(-0.743643887037151, 0.131825904205330)
        );
        assert!((zoom - 2.5).abs() < 1e-12);
    }
}
