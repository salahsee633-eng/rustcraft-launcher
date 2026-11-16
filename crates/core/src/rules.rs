//! Mojang version-JSON rule evaluation.
//!
//! The launcher pretends to be Linux on Android (that is what the JVM/game
//! expect), and selects native artifacts itself, so `os` rules only need the
//! handful of values Mojang actually emits.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: Action,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

/// What the current device "looks like" to the version JSON.
#[derive(Debug, Clone)]
pub struct Env {
    pub os_name: String,
    pub os_arch: String,
    pub os_version: String,
    pub features: HashMap<String, bool>,
}

impl Env {
    pub fn android() -> Self {
        Env {
            // Android is Linux as far as the game and LWJGL are concerned.
            os_name: "linux".into(),
            os_arch: std::env::consts::ARCH.into(),
            os_version: String::new(),
            features: HashMap::new(),
        }
    }

    pub fn desktop() -> Self {
        Env {
            os_name: match std::env::consts::OS {
                "macos" => "osx".into(),
                other => other.into(),
            },
            os_arch: std::env::consts::ARCH.into(),
            os_version: String::new(),
            features: HashMap::new(),
        }
    }

    pub fn feature(mut self, name: &str, value: bool) -> Self {
        self.features.insert(name.to_string(), value);
        self
    }
}

/// Standard Mojang semantics: empty rule list means "allowed", otherwise the
/// last matching rule wins and the default is "disallowed".
pub fn allowed(rules: &[Rule], env: &Env) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut result = false;
    for rule in rules {
        if rule_matches(rule, env) {
            result = rule.action == Action::Allow;
        }
    }
    result
}

fn rule_matches(rule: &Rule, env: &Env) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != &env.os_name {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            // Mojang uses "x86" for 32-bit x86 and "x86_64"/"arm64" elsewhere.
            if arch != &env.os_arch {
                return false;
            }
        }
        if let Some(pattern) = &os.version {
            if !version_matches(pattern, &env.os_version) {
                return false;
            }
        }
    }
    if let Some(features) = &rule.features {
        for (key, expected) in features {
            if env.features.get(key).copied().unwrap_or(false) != *expected {
                return false;
            }
        }
    }
    true
}

/// Mojang only uses anchored prefixes such as `^10\.` for Windows 10 checks,
/// which never match on Android. We support that small subset instead of
/// pulling a full regex engine into the app.
fn version_matches(pattern: &str, version: &str) -> bool {
    let simplified = pattern
        .trim_start_matches('^')
        .trim_end_matches('$')
        .replace("\\\\", "\\")
        .replace("\\.", ".");
    version.starts_with(simplified.trim_end_matches('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(json: &str) -> Rule {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn empty_rules_allow() {
        assert!(allowed(&[], &Env::android()));
    }

    #[test]
    fn linux_only_artifact_is_allowed_on_android() {
        let rules = vec![rule(r#"{"action":"allow","os":{"name":"linux"}}"#)];
        assert!(allowed(&rules, &Env::android()));
    }

    #[test]
    fn windows_artifact_is_rejected() {
        let rules = vec![rule(r#"{"action":"allow","os":{"name":"windows"}}"#)];
        assert!(!allowed(&rules, &Env::android()));
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = vec![
            rule(r#"{"action":"allow"}"#),
            rule(r#"{"action":"disallow","os":{"name":"linux"}}"#),
        ];
        assert!(!allowed(&rules, &Env::android()));
    }

    #[test]
    fn feature_rules() {
        let rules = vec![rule(
            r#"{"action":"allow","features":{"has_custom_resolution":true}}"#,
        )];
        assert!(!allowed(&rules, &Env::android()));
        assert!(allowed(&rules, &Env::android().feature("has_custom_resolution", true)));
    }
}
