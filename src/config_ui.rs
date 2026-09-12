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
    fields: Vec<ConfigField>,
}

impl ConfigUi {
    pub fn from_config(_config: &AppConfig) -> Self {
        todo!()
    }

    pub fn fields(&self) -> &[ConfigField] {
        &self.fields
    }
}
