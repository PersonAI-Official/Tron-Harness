//! Global native-tool exposure schemes and per-Agent overrides.
//!
//! A scheme is only an exposure choice: the run still intersects its tool IDs
//! with the admitted, authorized catalog. Scheme edits never grant authority.

use super::*;
use crate::domain::agent::{NativeToolConfig, NativeToolScheme};

const MAX_SCHEMES: usize = 32;
const MAX_TOOLS_PER_SCHEME: usize = 256;

pub(super) fn override_key(workspace_code: &str, agent_code: &str) -> String {
    format!("{workspace_code}::{agent_code}")
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn normalize_scheme(
    scheme: NativeToolScheme,
    built_in_codes: &std::collections::BTreeSet<String>,
) -> Result<NativeToolScheme, AgentManagementError> {
    let code = scheme.code.trim().to_ascii_lowercase();
    if !valid_code(&code) {
        return Err(AgentManagementError::InternalAgentPolicyInvalid);
    }
    let name = scheme.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AgentManagementError::InternalAgentPolicyInvalid);
    }
    let mut tool_ids = scheme
        .tool_ids
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    tool_ids.sort();
    tool_ids.dedup();
    if tool_ids.len() > MAX_TOOLS_PER_SCHEME {
        return Err(AgentManagementError::InternalAgentPolicyInvalid);
    }
    Ok(NativeToolScheme {
        built_in: built_in_codes.contains(&code),
        code,
        name,
        tool_ids,
    })
}

impl AgentManagementService {
    pub fn native_tool_config(&self) -> Result<NativeToolConfig, AgentManagementError> {
        let stored = self.repository().native_tool_config()?;
        let config = stored
            .as_deref()
            .and_then(|json| serde_json::from_str::<NativeToolConfig>(json).ok())
            .unwrap_or_default();
        self.normalize_config(config)
    }

    fn normalize_config(
        &self,
        mut config: NativeToolConfig,
    ) -> Result<NativeToolConfig, AgentManagementError> {
        if config.schemes.is_empty() || config.schemes.len() > MAX_SCHEMES {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        let built_in_codes = config
            .schemes
            .iter()
            .filter(|scheme| scheme.built_in)
            .map(|scheme| scheme.code.clone())
            .collect::<std::collections::BTreeSet<_>>();
        config.schemes = config
            .schemes
            .into_iter()
            .map(|scheme| normalize_scheme(scheme, &built_in_codes))
            .collect::<Result<Vec<_>, _>>()?;
        if config.schemes.iter().any(|scheme| scheme.code == "default") {
            for scheme in &mut config.schemes {
                scheme.built_in = scheme.code == "default";
            }
        }
        let codes = config
            .schemes
            .iter()
            .map(|scheme| scheme.code.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if codes.len() != config.schemes.len() {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        let default = config.default_scheme_code.trim().to_ascii_lowercase();
        if !codes.contains(default.as_str()) {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        config.default_scheme_code = default;
        config
            .agent_overrides
            .retain(|_, scheme| codes.contains(scheme.as_str()));
        Ok(config)
    }

    pub fn replace_native_tool_config(
        &self,
        schemes: Vec<NativeToolScheme>,
        default_scheme_code: String,
    ) -> Result<NativeToolConfig, AgentManagementError> {
        let current = self.native_tool_config()?;
        let config = self.normalize_config(NativeToolConfig {
            schemes,
            default_scheme_code,
            agent_overrides: current.agent_overrides,
        })?;
        let json =
            serde_json::to_string(&config).map_err(|_| AgentManagementError::StorageFailure)?;
        self.repository().replace_native_tool_config(&json)?;
        Ok(config)
    }

    pub fn agent_native_scheme(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<Option<String>, AgentManagementError> {
        Ok(self
            .native_tool_config()?
            .agent_overrides
            .get(&override_key(workspace_code, agent_code))
            .cloned())
    }

    pub fn set_agent_native_scheme(
        &self,
        workspace_code: &str,
        agent_code: &str,
        scheme_code: Option<&str>,
    ) -> Result<Option<String>, AgentManagementError> {
        self.active_workspace_detail(workspace_code)?;
        let mut config = self.native_tool_config()?;
        let key = override_key(workspace_code, agent_code);
        match scheme_code.map(str::trim).filter(|value| !value.is_empty()) {
            Some(code) => {
                let code = code.to_ascii_lowercase();
                if !config.schemes.iter().any(|scheme| scheme.code == code) {
                    return Err(AgentManagementError::InternalAgentPolicyInvalid);
                }
                config.agent_overrides.insert(key, code.clone());
                let json = serde_json::to_string(&config)
                    .map_err(|_| AgentManagementError::StorageFailure)?;
                self.repository().replace_native_tool_config(&json)?;
                Ok(Some(code))
            }
            None => {
                if config.agent_overrides.remove(&key).is_some() {
                    let json = serde_json::to_string(&config)
                        .map_err(|_| AgentManagementError::StorageFailure)?;
                    self.repository().replace_native_tool_config(&json)?;
                }
                Ok(None)
            }
        }
    }

    pub fn clear_agent_native_scheme(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<(), AgentManagementError> {
        self.set_agent_native_scheme(workspace_code, agent_code, None)
            .map(|_| ())
    }

    /// Scheme selected for one run, falling back to the global default. Tool IDs
    /// remain subject to admission and authorization in the Agent loop.
    pub fn native_tool_ids_for_agent(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<Vec<String>, AgentManagementError> {
        let config = self.native_tool_config()?;
        let selected = config
            .agent_overrides
            .get(&override_key(workspace_code, agent_code))
            .cloned()
            .unwrap_or_else(|| config.default_scheme_code.clone());
        let scheme = config
            .schemes
            .iter()
            .find(|scheme| scheme.code == selected)
            .or_else(|| {
                config
                    .schemes
                    .iter()
                    .find(|scheme| scheme.code == config.default_scheme_code)
            });
        Ok(scheme
            .map(|scheme| scheme.tool_ids.clone())
            .unwrap_or_default())
    }
}
