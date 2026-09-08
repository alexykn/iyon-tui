//! Typed finite configuration for occurrence-owned controls and roots.

use super::generated::{ControlKind, RootRole};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlConfig {
    pub multiline: bool,
    pub animation_interval_ms: Option<u32>,
}

impl ControlConfig {
    pub fn validate_for(self, kind: ControlKind) -> Result<(), ConfigError> {
        if self.multiline && kind != ControlKind::Editor {
            return Err(ConfigError::WrongKind);
        }
        if self
            .animation_interval_ms
            .is_some_and(|interval| interval == 0)
            || (self.animation_interval_ms.is_some() && kind != ControlKind::Animation)
        {
            return Err(ConfigError::WrongKind);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RootConfig {
    pub flow_boundary: u32,
    pub unit_identity: Option<u64>,
}

impl RootConfig {
    pub fn validate_for(self, role: RootRole) -> Result<(), ConfigError> {
        if self.flow_boundary > 1 {
            return Err(ConfigError::InvalidValue);
        }
        if role != RootRole::LegacyHistoryUnit
            && (self.flow_boundary != 0 || self.unit_identity.is_some())
        {
            return Err(ConfigError::WrongKind);
        }
        if self.unit_identity == Some(0) {
            return Err(ConfigError::InvalidValue);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    WrongKind,
    InvalidValue,
}
