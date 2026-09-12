//! Rendering method and coordinate-precision decisions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecisionTechnique {
    Float,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrecisionSpec {
    pub technique: PrecisionTechnique,
    pub level: usize,
}

impl PrecisionSpec {
    pub const fn float(level: usize) -> Self {
        Self {
            technique: PrecisionTechnique::Float,
            level,
        }
    }

    pub const fn fixed(level: usize) -> Self {
        Self {
            technique: PrecisionTechnique::Fixed,
            level,
        }
    }

    pub fn validate(self) -> Result<Self, &'static str> {
        if self.level == 0 {
            Err("precision level must be at least 1")
        } else {
            Ok(self)
        }
    }

    pub const fn normalized(self) -> Self {
        match self.technique {
            PrecisionTechnique::Float => Self::float(1),
            PrecisionTechnique::Fixed => self,
        }
    }

    pub fn validate_and_normalize(self) -> Result<Self, &'static str> {
        self.validate().map(|_| self.normalized())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMethod {
    Direct {
        precision: PrecisionSpec,
    },
    Perturbation {
        seed: PrecisionSpec,
        delta: PrecisionSpec,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrecisionRenderPlan {
    method: RenderMethod,
    perturbation_fallback: bool,
}

pub struct PrecisionDecisionManager;

impl PrecisionDecisionManager {
    pub fn from_config(
        config: &crate::config::RendererConfig,
    ) -> Result<PrecisionRenderPlan, String> {
        let parse = |technique: &str, level: usize| -> Result<PrecisionSpec, String> {
            let technique = match technique.to_ascii_lowercase().as_str() {
                "float" | "f64" => PrecisionTechnique::Float,
                "fixed" => PrecisionTechnique::Fixed,
                invalid => return Err(format!("unknown precision technique: {invalid}")),
            };
            PrecisionSpec { technique, level }
                .validate_and_normalize()
                .map_err(str::to_string)
        };
        let method = match config.rendering_method.as_str() {
            "multiprecision" => RenderMethod::Direct {
                precision: parse("fixed", 2)?,
            },
            "f64" | "direct" => RenderMethod::Direct {
                precision: parse(&config.precision_technique, config.precision_level)?,
            },
            "perturbation" => RenderMethod::Perturbation {
                seed: parse(&config.precision_technique, config.precision_level)?,
                delta: PrecisionSpec::float(1),
            },
            invalid => return Err(format!("unknown rendering method: {invalid}")),
        };
        Ok(PrecisionRenderPlan::new(method)
            .map_err(str::to_string)?
            .with_perturbation_fallback(config.perturbation_fallback))
    }
}

impl PrecisionRenderPlan {
    pub const fn direct(precision: PrecisionSpec) -> Self {
        Self {
            method: RenderMethod::Direct { precision },
            perturbation_fallback: false,
        }
    }

    pub const fn perturbation(seed: PrecisionSpec, delta: PrecisionSpec) -> Self {
        Self {
            method: RenderMethod::Perturbation { seed, delta },
            perturbation_fallback: false,
        }
    }

    pub fn new(method: RenderMethod) -> Result<Self, &'static str> {
        let method = match method {
            RenderMethod::Direct { precision } => {
                precision.validate_and_normalize()?;
                RenderMethod::Direct {
                    precision: precision.normalized(),
                }
            }
            RenderMethod::Perturbation { seed, delta } => {
                seed.validate_and_normalize()?;
                delta.validate_and_normalize()?;
                RenderMethod::Perturbation {
                    seed: seed.normalized(),
                    delta: delta.normalized(),
                }
            }
        };
        Ok(Self {
            method,
            perturbation_fallback: false,
        })
    }

    pub const fn method(self) -> RenderMethod {
        self.method
    }

    pub const fn with_perturbation_fallback(mut self, enabled: bool) -> Self {
        self.perturbation_fallback = enabled;
        self
    }

    pub const fn perturbation_fallback(self) -> bool {
        self.perturbation_fallback
    }
}

impl Default for PrecisionRenderPlan {
    fn default() -> Self {
        Self::direct(PrecisionSpec::float(1))
    }
}

#[cfg(test)]
mod tests {
    use super::{PrecisionRenderPlan, PrecisionSpec, PrecisionTechnique, RenderMethod};

    #[test]
    fn direct_plan_has_one_precision_spec() {
        let plan = PrecisionRenderPlan::new(RenderMethod::Direct {
            precision: PrecisionSpec::fixed(2),
        })
        .unwrap();

        assert_eq!(
            plan.method(),
            RenderMethod::Direct {
                precision: PrecisionSpec::fixed(2)
            }
        );
    }

    #[test]
    fn perturbation_plan_separates_seed_and_delta_precision() {
        let plan = PrecisionRenderPlan::new(RenderMethod::Perturbation {
            seed: PrecisionSpec::fixed(4),
            delta: PrecisionSpec::float(1),
        })
        .unwrap();

        assert_eq!(
            plan.method(),
            RenderMethod::Perturbation {
                seed: PrecisionSpec {
                    technique: PrecisionTechnique::Fixed,
                    level: 4
                },
                delta: PrecisionSpec::float(1),
            }
        );
    }

    #[test]
    fn precision_levels_start_at_one() {
        assert!(PrecisionRenderPlan::new(RenderMethod::Direct {
            precision: PrecisionSpec::fixed(0),
        })
        .is_err());
    }

    #[test]
    fn float_precision_level_is_normalized_to_one() {
        assert_eq!(
            PrecisionSpec::float(64).normalized(),
            PrecisionSpec::float(1)
        );
    }

    #[test]
    fn manager_uses_one_selector_for_perturbation_seed_and_f64_delta() {
        let mut config = crate::config::RendererConfig::default();
        config.rendering_method = "perturbation".to_string();
        config.precision_technique = "fixed".to_string();
        config.precision_level = 4;

        assert_eq!(
            super::PrecisionDecisionManager::from_config(&config)
                .unwrap()
                .method(),
            RenderMethod::Perturbation {
                seed: PrecisionSpec::fixed(4),
                delta: PrecisionSpec::float(1),
            }
        );
    }
}
