use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::rule::Severity;

/// Configuration loaded from `elm-assist.toml`.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Source directory override (default: "src").
    pub src: Option<String>,
    /// Rule configuration.
    #[serde(default)]
    pub rules: RulesConfig,
    /// TUI configuration.
    #[serde(default)]
    pub tui: TuiConfig,
}

/// TUI-specific configuration.
#[derive(Debug, Deserialize)]
pub struct TuiConfig {
    /// File watcher debounce interval in milliseconds (default: 200).
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

fn default_debounce_ms() -> u64 {
    200
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce_ms(),
        }
    }
}

/// Rule-level configuration.
#[derive(Debug, Default, Deserialize)]
pub struct RulesConfig {
    /// Rules to disable entirely.
    #[serde(default)]
    pub disable: Vec<String>,
    /// Per-rule severity overrides.
    #[serde(default)]
    pub severity: HashMap<String, SeverityValue>,
    /// Per-rule custom options. Any key under `[rules]` that isn't `disable` or
    /// `severity` is captured here as `RuleName -> toml::Value`.
    #[serde(flatten)]
    pub options: HashMap<String, toml::Value>,
}

/// A severity value in the config file. `Off` disables the rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeverityValue {
    Error,
    Warning,
    Off,
}

/// Errors that can occur when loading a config file.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "could not read config file: {e}"),
            ConfigError::Parse(e) => write!(f, "could not parse config file: {e}"),
        }
    }
}

impl Config {
    /// Load a config from a specific file path.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        toml::from_str(&contents).map_err(ConfigError::Parse)
    }

    /// Walk up from the current directory looking for `elm-assist.toml`.
    pub fn discover() -> Option<(PathBuf, Self)> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            let candidate = dir.join("elm-assist.toml");
            if candidate.exists() {
                let config = Self::load(&candidate).ok()?;
                return Some((candidate, config));
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    /// Check if a rule is disabled (by the `disable` list or `severity = "off"`).
    pub fn is_rule_disabled(&self, name: &str) -> bool {
        if self.rules.disable.iter().any(|n| n == name) {
            return true;
        }
        matches!(self.rules.severity.get(name), Some(SeverityValue::Off))
    }

    /// Get the configured severity for a rule, if any.
    pub fn severity_for(&self, name: &str) -> Option<Severity> {
        match self.rules.severity.get(name)? {
            SeverityValue::Error => Some(Severity::Error),
            SeverityValue::Warning => Some(Severity::Warning),
            SeverityValue::Off => None,
        }
    }

    /// Get per-rule options for a given rule name, if any.
    pub fn rule_options(&self, name: &str) -> Option<&toml::Value> {
        self.rules.options.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_better::prelude::*;

    #[test]
    fn parse_minimal_config() -> TestResult {
        let config: Config = toml::from_str("").or_fail_with("empty toml parses")?;
        check!(config.src.is_none()).satisfies(is_true())?;
        check!(config.rules.disable.is_empty()).satisfies(is_true())?;
        check!(config.rules.severity.is_empty()).satisfies(is_true())?;
        Ok(())
    }

    #[test]
    fn parse_full_config() -> TestResult {
        let config: Config = toml::from_str(
            r#"
src = "lib"

[rules]
disable = ["NoTodoComment", "NoMissingTypeAnnotation"]

[rules.severity]
NoDebug = "error"
NoUnusedImports = "warning"
NoAlwaysIdentity = "off"
"#,
        )
        .or_fail_with("config toml parses")?;

        check!(config.src.as_deref()).satisfies(eq(Some("lib")))?;
        check!(config.rules.disable.len()).satisfies(eq(2))?;
        check!(config.is_rule_disabled("NoTodoComment")).satisfies(is_true())?;
        check!(config.is_rule_disabled("NoMissingTypeAnnotation")).satisfies(is_true())?;
        check!(config.is_rule_disabled("NoDebug")).satisfies(is_false())?;
        // "off" in severity also disables.
        check!(config.is_rule_disabled("NoAlwaysIdentity")).satisfies(is_true())?;

        check!(config.severity_for("NoDebug")).satisfies(eq(Some(Severity::Error)))?;
        check!(config.severity_for("NoUnusedImports")).satisfies(eq(Some(Severity::Warning)))?;
        check!(config.severity_for("NoAlwaysIdentity")).satisfies(eq(None))?;
        check!(config.severity_for("UnknownRule")).satisfies(eq(None))?;
        Ok(())
    }

    #[test]
    fn default_config_disables_nothing() -> TestResult {
        let config = Config::default();
        check!(config.is_rule_disabled("NoDebug")).satisfies(is_false())?;
        check!(config.severity_for("NoDebug")).satisfies(eq(None))?;
        Ok(())
    }

    #[test]
    fn parse_per_rule_options() -> TestResult {
        let config: Config = toml::from_str(
            r#"
[rules.NoMaxLineLength]
max_length = 100

[rules.CognitiveComplexity]
threshold = 20

[rules.NoInconsistentAliases]
aliases = { "Json.Decode" = "Decode", "Html.Attributes" = "Attr" }
"#,
        )
        .or_fail_with("config toml parses")?;

        let max_opts = config
            .rule_options("NoMaxLineLength")
            .or_fail_with("NoMaxLineLength options present")?;
        check!(max_opts.get("max_length").and_then(|v| v.as_integer())).satisfies(eq(Some(100)))?;

        let cog_opts = config
            .rule_options("CognitiveComplexity")
            .or_fail_with("CognitiveComplexity options present")?;
        check!(cog_opts.get("threshold").and_then(|v| v.as_integer())).satisfies(eq(Some(20)))?;

        let alias_opts = config
            .rule_options("NoInconsistentAliases")
            .or_fail_with("NoInconsistentAliases options present")?;
        let aliases = alias_opts
            .get("aliases")
            .and_then(|v| v.as_table())
            .or_fail_with("aliases is a table")?;
        check!(aliases.get("Json.Decode").and_then(|v| v.as_str()))
            .satisfies(eq(Some("Decode")))?;
        check!(aliases.get("Html.Attributes").and_then(|v| v.as_str()))
            .satisfies(eq(Some("Attr")))?;

        // Unknown rules have no options.
        check!(config.rule_options("NoDebug").is_none()).satisfies(is_true())?;
        Ok(())
    }

    #[test]
    fn per_rule_options_coexist_with_disable_and_severity() -> TestResult {
        let config: Config = toml::from_str(
            r#"
[rules]
disable = ["NoTodoComment"]

[rules.severity]
NoDebug = "error"

[rules.NoMaxLineLength]
max_length = 80
"#,
        )
        .or_fail_with("config toml parses")?;

        check!(config.is_rule_disabled("NoTodoComment")).satisfies(is_true())?;
        check!(config.severity_for("NoDebug")).satisfies(eq(Some(Severity::Error)))?;
        let opts = config
            .rule_options("NoMaxLineLength")
            .or_fail_with("NoMaxLineLength options present")?;
        check!(opts.get("max_length").and_then(|v| v.as_integer())).satisfies(eq(Some(80)))?;
        Ok(())
    }
}
