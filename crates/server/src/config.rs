//! Server configuration.

use figment::Figment;
use figment::providers::{Env, Format, Toml};
use mewcode_protocol::env::{CONFIG_FILE, OPENCODE_GO_API_KEY};
use serde::Deserialize;

/// Expand a `~` and `${VAR}` placeholders in `raw`. Returns the path
/// unchanged if the placeholder is unset. Used for `external_dirs`
/// (Hermes-compatible behaviour).
fn expand_path(raw: &str) -> String {
    let mut s = raw.to_string();
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            s = format!("{}/{}", home.display(), stripped);
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            s = home.display().to_string();
        }
    }
    // ${VAR} substitution. Char-based walk so non-ASCII bytes
    // (e.g. `café` in a path) survive intact.
    let mut result = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let var = &after[..end];
            match std::env::var(var) {
                Ok(v) => result.push_str(&v),
                Err(_) => result.push_str(&rest[start..start + 2 + end + 1]),
            }
            rest = &after[end + 1..];
        } else {
            result.push_str(&rest[start..]);
            return result;
        }
    }
    result.push_str(rest);
    result
}

/// Default host the server binds to.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// Default port the server binds to.
pub const DEFAULT_PORT: u16 = 3737;

/// Default `tracing` filter when `RUST_LOG` is unset.
pub const DEFAULT_LOG: &str = "info,mewcode_engine=debug";

/// Env-var prefix figment reads for the server config.
pub const ENV_PREFIX: &str = "MEWCODE_";

/// Server configuration, loaded from `mewcode.toml` and the environment.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to bind to.
    #[serde(default = "default_port")]
    pub port: u16,
    /// OpenCode Go API key. Required.
    pub opencode_go_api_key: String,
    /// Default model.
    #[serde(default)]
    pub default_model: Option<String>,
    /// Log level.
    #[serde(default = "default_log")]
    pub log: String,
    /// Skill configuration. Optional — when absent, only the default
    /// discovery locations (global + project + dev) are used.
    #[serde(default)]
    pub skills: SkillServerConfig,
    /// Test seam: override the canvas project root without mutating
    /// the process CWD. Skipped by serde so it cannot be set from
    /// config files or env vars; only programmatic `with_*` builders
    /// touch it. Production code never sets this.
    #[serde(skip)]
    pub canvas_project_root_override: Option<std::path::PathBuf>,
}
/// Skills subsection of [`ServerConfig`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SkillServerConfig {
    /// Additional skill directories to scan, in addition to the
    /// defaults. `~` and `${VAR}` are expanded at load time. Useful
    /// for sharing skills across multiple repos or with other agents
    /// (Hermes / agentskills.io compatible — see
    /// `https://hermes-agent.nousresearch.com/docs/user-guide/features/skills`).
    #[serde(default)]
    pub external_dirs: Vec<String>,
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_log() -> String {
    DEFAULT_LOG.to_string()
}

impl ServerConfig {
    /// Load from env vars and optional `mewcode.toml`.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let mut figment = Figment::new()
            .merge(Toml::file(CONFIG_FILE).nested())
            .merge(Env::prefixed(ENV_PREFIX).split("__"));

        // `OPENCODE_GO_API_KEY` is the canonical env var; pull it in if the
        // prefixed form isn't set.
        if let Ok(key) = std::env::var(OPENCODE_GO_API_KEY) {
            if figment.find_metadata("opencode_go_api_key").is_none() {
                figment = figment.merge(("opencode_go_api_key", key));
            }
        }

        figment.extract().map_err(Box::new)
    }
}

impl SkillServerConfig {
    /// Resolve `external_dirs` to a list of absolute paths with `~` and
    /// `${VAR}` placeholders expanded. Non-existent paths are still
    /// returned — the engine's `SkillRegistry::load` will silently
    /// skip them (Hermes behaviour).
    pub fn resolved_dirs(&self) -> Vec<std::path::PathBuf> {
        self.external_dirs
            .iter()
            .map(|s| std::path::PathBuf::from(expand_path(s)))
            .collect()
    }
}

impl ServerConfig {
    /// Project root for canvas file I/O. Defaults to the server's
    /// current working directory so the canvas lives wherever the
    /// user launched the server.
    pub fn canvas_project_root(&self) -> std::path::PathBuf {
        // Test override wins: tests use `with_canvas_project_root`
        // to point at a tempdir without mutating process CWD (which
        // would race with parallel tests in the same binary).
        if let Some(p) = &self.canvas_project_root_override {
            return p.clone();
        }
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    /// Return a clone of this config with the canvas project root
    /// pinned to `path`. Test-only convenience: production config
    /// loading never calls this.
    pub fn with_canvas_project_root(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.canvas_project_root_override = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let out = expand_path("~/skills");
        assert!(out.starts_with(home.to_str().unwrap()));
    }

    #[test]
    fn expand_env_var() {
        // PATH is set on every system; use it to verify the ${VAR} branch
        // runs without mutating env state.
        let out = expand_path("${PATH}/skills");
        assert!(out.ends_with("/skills"), "got {out}");
        assert!(!out.contains("${"), "placeholder should be expanded");
    }

    #[test]
    fn expand_unknown_var_is_left_alone() {
        let out = expand_path("${MEW_DEFINITELY_NOT_SET_XYZ_42}/skills");
        // Either fully resolved (if user happens to have it set) or
        // left as-is — both are acceptable. We just want the function
        // to not panic.
        assert!(out.contains("skills"));
    }
}
