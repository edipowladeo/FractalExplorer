use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Checkbox,
    IntegerSpinner,
    FloatSpinner,
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigField {
    pub path: String,
    pub kind: ControlKind,
    pub value: toml::Value,
}

pub struct ConfigUi {
    document: toml::Value,
    fields: Vec<ConfigField>,
}

impl ConfigUi {
    pub fn from_config(config: &AppConfig) -> Self {
        let document = toml::Value::try_from(config).expect("AppConfig must serialize to TOML");
        let mut fields = Vec::new();
        collect_fields(&document, String::new(), &mut fields);
        Self { document, fields }
    }

    pub fn fields(&self) -> &[ConfigField] {
        &self.fields
    }

    pub fn toggle(&mut self, path: &str) -> bool {
        let Some(field) = self.fields.iter().find(|field| field.path == path) else {
            return false;
        };
        let Some(value) = field.value.as_bool() else {
            return false;
        };
        self.set_value(path, toml::Value::Boolean(!value))
    }

    pub fn step(&mut self, path: &str, direction: i8) -> bool {
        let Some(field) = self.fields.iter().find(|field| field.path == path) else {
            return false;
        };
        let value = match &field.value {
            toml::Value::Integer(value) => {
                toml::Value::Integer(value.saturating_add(direction as i64))
            }
            toml::Value::Float(value) => toml::Value::Float(value + direction as f64),
            _ => return false,
        };
        self.set_value(path, value)
    }

    pub fn apply_to(&self, config: &mut AppConfig) -> Result<(), toml::de::Error> {
        *config = self.document.clone().try_into()?;
        Ok(())
    }

    fn set_value(&mut self, path: &str, value: toml::Value) -> bool {
        if !is_valid_value(path, &value) {
            return false;
        }
        if !set_document_value(&mut self.document, path, value.clone()) {
            return false;
        }
        let Some(field) = self.fields.iter_mut().find(|field| field.path == path) else {
            return false;
        };
        field.value = value;
        true
    }

    #[cfg(feature = "native-ui")]
    pub fn show(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        let mut changes = Vec::new();
        eframe::egui::ScrollArea::vertical().show(ui, |ui| {
            for field in &self.fields {
                let path = field.path.clone();
                let value = field.value.clone();
                match field.kind {
                    ControlKind::Checkbox => {
                        let mut checked = value.as_bool().expect("checkbox must be boolean");
                        if ui.checkbox(&mut checked, &path).changed() {
                            changes.push((path, toml::Value::Boolean(checked)));
                        }
                    }
                    ControlKind::IntegerSpinner => {
                        let mut number = value.as_integer().expect("spinner must be integer");
                        ui.horizontal(|ui| {
                            ui.label(&path);
                            let mut spinner = eframe::egui::DragValue::new(&mut number);
                            if path == "renderer.precision_level" {
                                spinner = spinner.range(1..=i64::MAX);
                            }
                            let mut changed = ui.add(spinner).changed();
                            if path == "renderer.precision_level" {
                                if ui.small_button("−").clicked() {
                                    if let Some(next) = precision_step(number, -1) {
                                        number = next;
                                        changed = true;
                                    }
                                }
                                if ui.small_button("+").clicked() {
                                    if let Some(next) = precision_step(number, 1) {
                                        number = next;
                                        changed = true;
                                    }
                                }
                            }
                            if changed {
                                changes.push((path.clone(), toml::Value::Integer(number)));
                            }
                        });
                    }
                    ControlKind::FloatSpinner => {
                        let mut number = value.as_float().expect("spinner must be float");
                        ui.horizontal(|ui| {
                            ui.label(&path);
                            if ui
                                .add(eframe::egui::DragValue::new(&mut number).speed(0.1))
                                .changed()
                            {
                                changes.push((path.clone(), toml::Value::Float(number)));
                            }
                        });
                    }
                    ControlKind::ReadOnly => {
                        ui.label(format!("{path}: {value}"));
                    }
                }
            }
        });
        let changed = !changes.is_empty();
        for (path, value) in changes {
            self.set_value(&path, value);
        }
        changed
    }
}

#[cfg(feature = "native-ui")]
pub fn run_window(
    config: &mut AppConfig,
    renderer_updates: std::sync::mpsc::Sender<crate::config::RendererConfig>,
) -> eframe::Result<()> {
    use std::sync::{Arc, Mutex};

    let state = Arc::new(Mutex::new(ConfigWindowState {
        config: config.clone(),
        fields: ConfigUi::from_config(config),
        renderer_updates,
    }));
    let app_state = Arc::clone(&state);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("FractalExplorer - Configuração")
            .with_inner_size([560.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FractalExplorer - Configuração",
        options,
        Box::new(move |_creation_context| Ok(Box::new(ConfigWindow { state: app_state }))),
    )?;
    state
        .lock()
        .expect("config UI mutex poisoned")
        .fields
        .apply_to(config)
        .expect("config UI state must remain compatible with AppConfig");
    Ok(())
}

#[cfg(feature = "native-ui")]
struct ConfigWindow {
    state: std::sync::Arc<std::sync::Mutex<ConfigWindowState>>,
}

#[cfg(feature = "native-ui")]
struct ConfigWindowState {
    config: AppConfig,
    fields: ConfigUi,
    renderer_updates: std::sync::mpsc::Sender<crate::config::RendererConfig>,
}

#[cfg(feature = "native-ui")]
impl eframe::App for ConfigWindow {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock().expect("config UI mutex poisoned");
        if state.fields.show(ui) {
            let mut updated_config = state.config.clone();
            state
                .fields
                .apply_to(&mut updated_config)
                .expect("config UI state must remain compatible with AppConfig");
            state.config = updated_config;
            let _ = state.renderer_updates.send(state.config.renderer.clone());
        }
    }
}

fn collect_fields(value: &toml::Value, prefix: String, fields: &mut Vec<ConfigField>) {
    let Some(table) = value.as_table() else {
        return;
    };

    for (name, child) in table {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };

        if child.is_table() {
            collect_fields(child, path, fields);
        } else {
            fields.push(ConfigField {
                path,
                kind: control_kind(child),
                value: child.clone(),
            });
        }
    }
}

fn control_kind(value: &toml::Value) -> ControlKind {
    match value {
        toml::Value::Boolean(_) => ControlKind::Checkbox,
        toml::Value::Integer(_) => ControlKind::IntegerSpinner,
        toml::Value::Float(_) => ControlKind::FloatSpinner,
        _ => ControlKind::ReadOnly,
    }
}

fn set_document_value(document: &mut toml::Value, path: &str, value: toml::Value) -> bool {
    let mut current = document;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let Some(table) = current.as_table_mut() else {
                return false;
            };
            if !table.contains_key(part) {
                return false;
            }
            table.insert(part.to_string(), value);
            return true;
        }
        let Some(child) = current.as_table_mut().and_then(|table| table.get_mut(part)) else {
            return false;
        };
        current = child;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{precision_step, ConfigUi, ControlKind};
    use crate::config::AppConfig;

    #[test]
    fn discovers_nested_properties_and_assigns_control_kinds() {
        let ui = ConfigUi::from_config(&AppConfig::default());

        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "debug_global")
                .unwrap()
                .kind,
            ControlKind::Checkbox
        );
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.width")
                .unwrap()
                .kind,
            ControlKind::IntegerSpinner
        );
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.precision_level")
                .unwrap()
                .kind,
            ControlKind::IntegerSpinner
        );
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.precision_level")
                .unwrap()
                .value
                .as_integer(),
            Some(1)
        );
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.allocation_ratio")
                .unwrap()
                .kind,
            ControlKind::FloatSpinner
        );
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.debug.reduced_viewport")
                .unwrap()
                .kind,
            ControlKind::Checkbox
        );
    }

    #[test]
    fn exposes_unsupported_scalar_properties_as_read_only() {
        let ui = ConfigUi::from_config(&AppConfig::default());

        let field = ui
            .fields()
            .iter()
            .find(|field| field.path == "renderer.starting_point")
            .unwrap();
        assert_eq!(field.kind, ControlKind::ReadOnly);
        assert!(field.value.as_str().is_some());
    }

    #[test]
    fn edits_boolean_integer_and_float_values_and_applies_them() {
        let mut ui = ConfigUi::from_config(&AppConfig::default());
        let mut config = AppConfig::default();

        assert!(ui.toggle("debug_global"));
        assert!(ui.step("renderer.width", 1));
        assert!(ui.step("renderer.allocation_ratio", -1));
        ui.apply_to(&mut config).unwrap();

        assert!(config.debug_global);
        assert_eq!(config.renderer.width, 641);
        assert!((config.renderer.allocation_ratio - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn rejects_edits_to_read_only_or_unknown_properties() {
        let mut ui = ConfigUi::from_config(&AppConfig::default());

        assert!(!ui.toggle("renderer.starting_point"));
        assert!(!ui.step("renderer.palette", 1));
        assert!(!ui.step("renderer.missing", 1));
    }

    #[test]
    fn rejects_non_positive_renderer_precision() {
        let mut ui = ConfigUi::from_config(&AppConfig::default());

        assert!(!ui.step("renderer.precision_level", -1));
        assert_eq!(
            ui.fields()
                .iter()
                .find(|field| field.path == "renderer.precision_level")
                .unwrap()
                .value
                .as_integer(),
            Some(1)
        );
    }

    #[test]
    fn precision_buttons_step_up_and_down_without_reaching_zero() {
        assert_eq!(precision_step(1, -1), None);
        assert_eq!(precision_step(1, 1), Some(2));
        assert_eq!(precision_step(i64::MAX, 1), None);
    }
}

fn is_valid_value(path: &str, value: &toml::Value) -> bool {
    path != "renderer.precision_level" || value.as_integer().is_some_and(|value| value > 0)
}

fn precision_step(value: i64, direction: i8) -> Option<i64> {
    let next = value.saturating_add(direction as i64);
    (next > 0 && next != value).then_some(next)
}
