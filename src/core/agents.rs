//! agent → skills directory mapping table (static data).
//!
//! Data-driven design: one config line per agent; detection/dir resolution goes through the
//! injectable [`Env`], making unit tests easy (build a temp dir, no touching the real environment).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The common skills dir shared by most agents.
pub const UNIVERSAL_SKILLS_DIR: &str = ".agents/skills";

/// Context for agent detection/dir resolution (owns data, easy to inject in tests).
pub struct Env {
    /// Home directory (`~`).
    pub home: PathBuf,
    /// Config directory (`$XDG_CONFIG_HOME` or `~/.config`).
    pub config: PathBuf,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Environment variables injectable in tests (reads the real env when None).
    vars: Option<HashMap<String, String>>,
}

impl Env {
    /// Construct an environment context from explicit paths.
    pub fn new(home: impl AsRef<Path>, config: impl AsRef<Path>, cwd: impl AsRef<Path>) -> Self {
        Env {
            home: home.as_ref().to_path_buf(),
            config: config.as_ref().to_path_buf(),
            cwd: cwd.as_ref().to_path_buf(),
            vars: None,
        }
    }

    /// Read an environment variable (injectable override in tests).
    pub fn var(&self, key: &str) -> Option<String> {
        match &self.vars {
            Some(map) => map.get(key).cloned(),
            None => std::env::var(key).ok(),
        }
    }

    /// Inject environment variable overrides (for library users and tests).
    pub fn set_vars(&mut self, vars: HashMap<String, String>) {
        self.vars = Some(vars);
    }
}

/// config dir: `$XDG_CONFIG_HOME || ~/.config`.
pub fn config_home() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => home().join(".config"),
    }
}

/// Resolve the user's home directory.
pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// Global skills directory base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalDir {
    /// Home-based.
    Home(&'static str),
    /// Config-based.
    Config(&'static str),
    /// Based on an env-var agent home (e.g. `~/.claude`).
    Env(EnvKey, &'static str),
    /// No global install.
    None,
}

/// An agent home key supporting `$VAR || ~/.<dir>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvKey {
    /// Claude agent home.
    Claude,
    /// Codex agent home.
    Codex,
    /// Grok agent home.
    Grok,
    /// Hermes agent home.
    Hermes,
    /// Vibe agent home.
    Vibe,
    /// Autohand agent home.
    Autohand,
}

/// Install detection method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detect {
    /// `home/<path>` exists.
    Home(&'static str),
    /// `config/<path>` exists.
    Config(&'static str),
    /// `cwd/<path>` exists.
    Cwd(&'static str),
    /// `home/<path>` or `cwd/<path>` exists.
    HomeOrCwd(&'static str),
    /// Special rule.
    Special(SpecialKey),
}

/// Special detection rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
    /// OpenClaw.
    Openclaw,
    /// Eve.
    Eve,
    /// ZCode.
    Zcode,
    /// MiniMax.
    Minimax,
    /// AstrBot.
    Astrbot,
    /// Zed.
    Zed,
    /// PromptScript.
    Promptscript,
    /// Kimi.
    Kimi,
    /// Universal skills.
    Universal,
    /// Home-dir detection via an EnvKey.
    Env(EnvKey),
}

/// An agent's directory config.
#[derive(Debug, Clone, Copy)]
pub struct Agent {
    /// Agent identifier (used on the CLI).
    pub name: &'static str,
    /// Human-readable display name.
    pub display: &'static str,
    /// Project-level skills dir (relative to cwd).
    pub skills_dir: &'static str,
    /// Global skills directory.
    pub global: GlobalDir,
    /// Install detection rule.
    pub detect: Detect,
    /// Whether it appears in the universal agents list (default true).
    pub show_in_universal_list: bool,
}

impl Agent {
    /// Construct an agent config.
    pub const fn new(
        name: &'static str,
        display: &'static str,
        skills_dir: &'static str,
        global: GlobalDir,
        detect: Detect,
    ) -> Self {
        Agent {
            name,
            display,
            skills_dir,
            global,
            detect,
            show_in_universal_list: true,
        }
    }

    /// Whether it uses the common `.agents/skills` dir (no symlink needed).
    pub fn is_universal(&self) -> bool {
        self.skills_dir == UNIVERSAL_SKILLS_DIR
    }
}

/// Agent static table (all 70+ agents).
pub const AGENTS: &[Agent] = &[
    Agent::new(
        "aider-desk",
        "AiderDesk",
        ".aider-desk/skills",
        GlobalDir::Home(".aider-desk/skills"),
        Detect::Home(".aider-desk"),
    ),
    Agent::new(
        "amp",
        "Amp",
        ".agents/skills",
        GlobalDir::Config("agents/skills"),
        Detect::Config("amp"),
    ),
    Agent::new(
        "antigravity",
        "Antigravity",
        ".agents/skills",
        GlobalDir::Home(".gemini/antigravity/skills"),
        Detect::Home(".gemini/antigravity"),
    ),
    Agent::new(
        "antigravity-cli",
        "Antigravity CLI",
        ".agents/skills",
        GlobalDir::Home(".gemini/antigravity-cli/skills"),
        Detect::Home(".gemini/antigravity-cli"),
    ),
    Agent::new(
        "astrbot",
        "AstrBot",
        "data/skills",
        GlobalDir::Home(".astrbot/data/skills"),
        Detect::Special(SpecialKey::Astrbot),
    ),
    Agent::new(
        "autohand-code",
        "Autohand Code CLI",
        ".autohand/skills",
        GlobalDir::Env(EnvKey::Autohand, "skills"),
        Detect::Special(SpecialKey::Env(EnvKey::Autohand)),
    ),
    Agent::new(
        "augment",
        "Augment",
        ".augment/skills",
        GlobalDir::Home(".augment/skills"),
        Detect::Home(".augment"),
    ),
    Agent::new(
        "bob",
        "IBM Bob",
        ".bob/skills",
        GlobalDir::Home(".bob/skills"),
        Detect::Home(".bob"),
    ),
    Agent::new(
        "claude-code",
        "Claude Code",
        ".claude/skills",
        GlobalDir::Env(EnvKey::Claude, "skills"),
        Detect::Special(SpecialKey::Claude),
    ),
    Agent::new(
        "openclaw",
        "OpenClaw",
        "skills",
        GlobalDir::Home(".openclaw/skills"),
        Detect::Special(SpecialKey::Openclaw),
    ),
    Agent::new(
        "cline",
        "Cline",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Home(".cline"),
    ),
    Agent::new(
        "codearts-agent",
        "CodeArts Agent",
        ".codeartsdoer/skills",
        GlobalDir::Home(".codeartsdoer/skills"),
        Detect::Home(".codeartsdoer"),
    ),
    Agent::new(
        "codebuddy",
        "CodeBuddy",
        ".codebuddy/skills",
        GlobalDir::Home(".codebuddy/skills"),
        Detect::HomeOrCwd(".codebuddy"),
    ),
    Agent::new(
        "codemaker",
        "Codemaker",
        ".codemaker/skills",
        GlobalDir::Home(".codemaker/skills"),
        Detect::Home(".codemaker"),
    ),
    Agent::new(
        "codestudio",
        "Code Studio",
        ".codestudio/skills",
        GlobalDir::Home(".codestudio/skills"),
        Detect::Home(".codestudio"),
    ),
    Agent::new(
        "codex",
        "Codex",
        ".agents/skills",
        GlobalDir::Env(EnvKey::Codex, "skills"),
        Detect::Special(SpecialKey::Codex),
    ),
    Agent::new(
        "command-code",
        "Command Code",
        ".commandcode/skills",
        GlobalDir::Home(".commandcode/skills"),
        Detect::Home(".commandcode"),
    ),
    Agent::new(
        "continue",
        "Continue",
        ".continue/skills",
        GlobalDir::Home(".continue/skills"),
        Detect::HomeOrCwd(".continue"),
    ),
    Agent::new(
        "cortex",
        "Cortex Code",
        ".cortex/skills",
        GlobalDir::Home(".snowflake/cortex/skills"),
        Detect::Home(".snowflake/cortex"),
    ),
    Agent::new(
        "crush",
        "Crush",
        ".crush/skills",
        GlobalDir::Home(".config/crush/skills"),
        Detect::Home(".config/crush"),
    ),
    Agent::new(
        "cursor",
        "Cursor",
        ".agents/skills",
        GlobalDir::Home(".cursor/skills"),
        Detect::Home(".cursor"),
    ),
    Agent::new(
        "deepagents",
        "Deep Agents",
        ".agents/skills",
        GlobalDir::Home(".deepagents/agent/skills"),
        Detect::Home(".deepagents"),
    ),
    Agent::new(
        "devin",
        "Devin for Terminal",
        ".devin/skills",
        GlobalDir::Config("devin/skills"),
        Detect::Config("devin"),
    ),
    Agent::new(
        "dexto",
        "Dexto",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Home(".dexto"),
    )
    .mark(PrivateMark::Dexto),
    Agent::new(
        "droid",
        "Droid",
        ".factory/skills",
        GlobalDir::Home(".factory/skills"),
        Detect::Home(".factory"),
    ),
    Agent::new(
        "eve",
        "Eve",
        "agent/skills",
        GlobalDir::None,
        Detect::Special(SpecialKey::Eve),
    ),
    Agent::new(
        "firebender",
        "Firebender",
        ".agents/skills",
        GlobalDir::Home(".firebender/skills"),
        Detect::Home(".firebender"),
    )
    .mark(PrivateMark::Firebender),
    Agent::new(
        "forgecode",
        "ForgeCode",
        ".forge/skills",
        GlobalDir::Home(".forge/skills"),
        Detect::Home(".forge"),
    ),
    Agent::new(
        "gemini-cli",
        "Gemini CLI",
        ".agents/skills",
        GlobalDir::Home(".gemini/skills"),
        Detect::Home(".gemini"),
    ),
    Agent::new(
        "github-copilot",
        "GitHub Copilot",
        ".agents/skills",
        GlobalDir::Home(".copilot/skills"),
        Detect::Home(".copilot"),
    ),
    Agent::new(
        "goose",
        "Goose",
        ".goose/skills",
        GlobalDir::Config("goose/skills"),
        Detect::Config("goose"),
    ),
    Agent::new(
        "grok",
        "Grok Build",
        ".grok/skills",
        GlobalDir::Env(EnvKey::Grok, "skills"),
        Detect::Special(SpecialKey::Env(EnvKey::Grok)),
    ),
    Agent::new(
        "hermes-agent",
        "Hermes Agent",
        ".hermes/skills",
        GlobalDir::Env(EnvKey::Hermes, "skills"),
        Detect::Special(SpecialKey::Env(EnvKey::Hermes)),
    ),
    Agent::new(
        "inference-sh",
        "inference.sh",
        ".inferencesh/skills",
        GlobalDir::Home(".inferencesh/skills"),
        Detect::Home(".inferencesh"),
    ),
    Agent::new(
        "iflow-cli",
        "iFlow CLI",
        ".iflow/skills",
        GlobalDir::Home(".iflow/skills"),
        Detect::Home(".iflow"),
    ),
    Agent::new(
        "jazz",
        "Jazz",
        ".jazz/skills",
        GlobalDir::Home(".jazz/skills"),
        Detect::HomeOrCwd(".jazz"),
    ),
    Agent::new(
        "junie",
        "Junie",
        ".junie/skills",
        GlobalDir::Home(".junie/skills"),
        Detect::Home(".junie"),
    ),
    Agent::new(
        "kilo",
        "Kilo Code",
        ".kilocode/skills",
        GlobalDir::Home(".kilocode/skills"),
        Detect::Home(".kilocode"),
    ),
    Agent::new(
        "kimchi",
        "Kimchi",
        ".kimchi/skills",
        GlobalDir::Home(".config/kimchi/harness/skills"),
        Detect::Home(".config/kimchi"),
    ),
    Agent::new(
        "kimi-code-cli",
        "Kimi Code CLI",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Special(SpecialKey::Kimi),
    ),
    Agent::new(
        "kiro-cli",
        "Kiro CLI",
        ".kiro/skills",
        GlobalDir::Home(".kiro/skills"),
        Detect::Home(".kiro"),
    ),
    Agent::new(
        "kode",
        "Kode",
        ".kode/skills",
        GlobalDir::Home(".kode/skills"),
        Detect::Home(".kode"),
    ),
    Agent::new(
        "lingma",
        "Lingma",
        ".lingma/skills",
        GlobalDir::Home(".lingma/skills"),
        Detect::Home(".lingma"),
    ),
    Agent::new(
        "loaf",
        "Loaf",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Home(".loaf"),
    )
    .mark(PrivateMark::Loaf),
    Agent::new(
        "mcpjam",
        "MCPJam",
        ".mcpjam/skills",
        GlobalDir::Home(".mcpjam/skills"),
        Detect::Home(".mcpjam"),
    ),
    Agent::new(
        "minimax-code",
        "MiniMax Code",
        ".minimax/skills",
        GlobalDir::Home(".minimax/skills"),
        Detect::Special(SpecialKey::Minimax),
    ),
    Agent::new(
        "mistral-vibe",
        "Mistral Vibe",
        ".vibe/skills",
        GlobalDir::Env(EnvKey::Vibe, "skills"),
        Detect::Special(SpecialKey::Env(EnvKey::Vibe)),
    ),
    Agent::new(
        "moxby",
        "Moxby",
        ".moxby/skills",
        GlobalDir::Home(".moxby/skills"),
        Detect::Home(".moxby"),
    ),
    Agent::new(
        "mux",
        "Mux",
        ".mux/skills",
        GlobalDir::Home(".mux/skills"),
        Detect::Home(".mux"),
    ),
    Agent::new(
        "neovate",
        "Neovate",
        ".neovate/skills",
        GlobalDir::Home(".neovate/skills"),
        Detect::Home(".neovate"),
    ),
    Agent::new(
        "opencode",
        "OpenCode",
        ".agents/skills",
        GlobalDir::Config("opencode/skills"),
        Detect::Config("opencode"),
    ),
    Agent::new(
        "openhands",
        "OpenHands",
        ".openhands/skills",
        GlobalDir::Home(".openhands/skills"),
        Detect::Home(".openhands"),
    ),
    Agent::new(
        "ona",
        "Ona",
        ".ona/skills",
        GlobalDir::Home(".ona/skills"),
        Detect::Home(".ona"),
    ),
    Agent::new(
        "pi",
        "Pi",
        ".pi/skills",
        GlobalDir::Home(".pi/agent/skills"),
        Detect::Home(".pi/agent"),
    ),
    Agent::new(
        "qoder",
        "Qoder",
        ".qoder/skills",
        GlobalDir::Home(".qoder/skills"),
        Detect::Home(".qoder"),
    ),
    Agent::new(
        "qoder-cn",
        "Qoder CN",
        ".qoder/skills",
        GlobalDir::Home(".qoder-cn/skills"),
        Detect::Home(".qoder-cn"),
    ),
    Agent::new(
        "qwen-code",
        "Qwen Code",
        ".qwen/skills",
        GlobalDir::Home(".qwen/skills"),
        Detect::Home(".qwen"),
    ),
    Agent::new(
        "replit",
        "Replit",
        ".agents/skills",
        GlobalDir::Config("agents/skills"),
        Detect::Cwd(".replit"),
    )
    .mark(PrivateMark::Replit),
    Agent::new(
        "reasonix",
        "Reasonix",
        ".reasonix/skills",
        GlobalDir::Home(".reasonix/skills"),
        Detect::Home(".reasonix"),
    ),
    Agent::new(
        "roo",
        "Roo Code",
        ".roo/skills",
        GlobalDir::Home(".roo/skills"),
        Detect::Home(".roo"),
    ),
    Agent::new(
        "rovodev",
        "Rovo Dev",
        ".rovodev/skills",
        GlobalDir::Home(".rovodev/skills"),
        Detect::Home(".rovodev"),
    ),
    Agent::new(
        "tabnine-cli",
        "Tabnine CLI",
        ".tabnine/agent/skills",
        GlobalDir::Home(".tabnine/agent/skills"),
        Detect::Home(".tabnine"),
    ),
    Agent::new(
        "terramind",
        "Terramind",
        ".terramind/skills",
        GlobalDir::Home(".terramind/skills"),
        Detect::Home(".terramind"),
    ),
    Agent::new(
        "tinycloud",
        "Tinycloud",
        ".tinycloud/skills",
        GlobalDir::Home(".tinycloud/skills"),
        Detect::Home(".tinycloud"),
    ),
    Agent::new(
        "trae",
        "Trae",
        ".trae/skills",
        GlobalDir::Home(".trae/skills"),
        Detect::Home(".trae"),
    ),
    Agent::new(
        "trae-cn",
        "Trae CN",
        ".trae/skills",
        GlobalDir::Home(".trae-cn/skills"),
        Detect::Home(".trae-cn"),
    ),
    Agent::new(
        "warp",
        "Warp",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Home(".warp"),
    ),
    Agent::new(
        "windsurf",
        "Windsurf",
        ".windsurf/skills",
        GlobalDir::Home(".codeium/windsurf/skills"),
        Detect::Home(".codeium/windsurf"),
    ),
    Agent::new(
        "zed",
        "Zed",
        ".agents/skills",
        GlobalDir::Home(".agents/skills"),
        Detect::Special(SpecialKey::Zed),
    ),
    Agent::new(
        "zcode",
        "ZCode",
        ".zcode/skills",
        GlobalDir::Home(".zcode/skills"),
        Detect::Special(SpecialKey::Zcode),
    ),
    Agent::new(
        "zencoder",
        "Zencoder",
        ".zencoder/skills",
        GlobalDir::Home(".zencoder/skills"),
        Detect::Home(".zencoder"),
    ),
    Agent::new(
        "zenflow",
        "Zenflow",
        ".zencoder/skills",
        GlobalDir::Home(".zencoder/skills"),
        Detect::Home(".zencoder"),
    ),
    Agent::new(
        "pochi",
        "Pochi",
        ".pochi/skills",
        GlobalDir::Home(".pochi/skills"),
        Detect::Home(".pochi"),
    ),
    Agent::new(
        "promptscript",
        "PromptScript",
        ".agents/skills",
        GlobalDir::None,
        Detect::Special(SpecialKey::Promptscript),
    )
    .mark(PrivateMark::Promptscript),
    Agent::new(
        "adal",
        "AdaL",
        ".adal/skills",
        GlobalDir::Home(".adal/skills"),
        Detect::Home(".adal"),
    ),
    Agent::new(
        "universal",
        "Universal",
        ".agents/skills",
        GlobalDir::Config("agents/skills"),
        Detect::Special(SpecialKey::Universal),
    )
    .mark(PrivateMark::Universal),
];

impl Agent {
    /// Turn off show_in_universal_list (for a few special agents).
    const fn mark(mut self, _: PrivateMark) -> Self {
        self.show_in_universal_list = false;
        self
    }
}

/// Private marker, used only to turn off show_in_universal_list.
enum PrivateMark {
    Dexto,
    Firebender,
    Loaf,
    Replit,
    Promptscript,
    Universal,
}

/// Look up an agent by name.
pub fn get_agent(name: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|a| a.name == name)
}

/// Display name of an agent (falls back to the raw name).
pub fn agent_display(name: &str) -> String {
    get_agent(name)
        .map(|a| a.display.to_string())
        .unwrap_or_else(|| name.to_string())
}

/// Agents using the common dir (no symlink; excludes those with show_in_universal_list false).
pub fn universal_agents() -> Vec<&'static Agent> {
    AGENTS
        .iter()
        .filter(|a| a.is_universal() && a.show_in_universal_list)
        .collect()
}

/// Agents not using the common dir (need symlinks). Used by remove/update.
#[allow(dead_code)]
pub fn non_universal_agents() -> Vec<&'static Agent> {
    AGENTS.iter().filter(|a| !a.is_universal()).collect()
}

/// An agent's global skills dir (None when global is unsupported).
pub fn global_skills_dir(agent: &Agent, env: &Env) -> Option<PathBuf> {
    match agent.global {
        GlobalDir::Home(p) => Some(env.home.join(p)),
        GlobalDir::Config(p) => Some(env.config.join(p)),
        GlobalDir::Env(key, p) => Some(env_home(key, env).join(p)),
        GlobalDir::None => None,
    }
}

/// `$VAR || home/.<default>` (per-agent home conventions).
pub fn env_home(key: EnvKey, env: &Env) -> PathBuf {
    let (var, default) = match key {
        EnvKey::Claude => ("CLAUDE_CONFIG_DIR", ".claude"),
        EnvKey::Codex => ("CODEX_HOME", ".codex"),
        EnvKey::Grok => ("GROK_HOME", ".grok"),
        EnvKey::Hermes => ("HERMES_HOME", ".hermes"),
        EnvKey::Vibe => ("VIBE_HOME", ".vibe"),
        EnvKey::Autohand => ("AUTOHAND_HOME", ".autohand"),
    };
    match env.var(var) {
        Some(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => env.home.join(default),
    }
}

/// Determine whether an agent is installed.
pub fn is_installed(agent: &Agent, env: &Env) -> bool {
    match agent.detect {
        Detect::Home(p) => env.home.join(p).exists(),
        Detect::Config(p) => env.config.join(p).exists(),
        Detect::Cwd(p) => env.cwd.join(p).exists(),
        Detect::HomeOrCwd(p) => env.home.join(p).exists() || env.cwd.join(p).exists(),
        Detect::Special(k) => special_detect(k, env),
    }
}

fn special_detect(k: SpecialKey, env: &Env) -> bool {
    match k {
        SpecialKey::Claude => env_home(EnvKey::Claude, env).exists(),
        SpecialKey::Codex => {
            env_home(EnvKey::Codex, env).exists() || Path::new("/etc/codex").exists()
        }
        SpecialKey::Openclaw => {
            env.home.join(".openclaw").exists()
                || env.home.join(".clawdbot").exists()
                || env.home.join(".moltbot").exists()
        }
        SpecialKey::Eve => env.cwd.join("agent").exists() && has_dependency(&env.cwd, "eve"),
        SpecialKey::Zcode => {
            env.home.join(".zcode").exists() || Path::new("/Applications/ZCode.app").exists()
        }
        SpecialKey::Minimax => {
            env.home.join(".minimax").exists()
                || Path::new("/Applications/MiniMax Code.app").exists()
        }
        SpecialKey::Astrbot => {
            env.cwd.join("data/skills").exists() || env.home.join(".astrbot").exists()
        }
        SpecialKey::Zed => {
            env.config.join("zed").exists()
                || env
                    .var("APPDATA")
                    .map(|a| Path::new(&a).join("Zed").exists())
                    .unwrap_or(false)
        }
        SpecialKey::Promptscript => {
            env.cwd.join(".promptscript").exists() || env.cwd.join("promptscript.yaml").exists()
        }
        SpecialKey::Kimi => env.home.join(".kimi-code").exists() || env.home.join(".kimi").exists(),
        SpecialKey::Universal => false,
        SpecialKey::Env(key) => env_home(key, env).exists(),
    }
}

/// Detect currently installed agents (universal is never detected as installed).
pub fn detect_installed_agents(env: &Env) -> Vec<&'static Agent> {
    AGENTS.iter().filter(|a| is_installed(a, env)).collect()
}

/// Ensure universal agents are present (append those missing from the target list).
pub fn ensure_universal_agents(mut target: Vec<&'static Agent>) -> Vec<&'static Agent> {
    for a in AGENTS.iter().filter(|a| a.is_universal()) {
        if !target.iter().any(|x| x.name == a.name) {
            target.push(a);
        }
    }
    target
}

/// Read package.json to check a declared dependency (used for eve detection).
fn has_dependency(cwd: &Path, name: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(cwd.join("package.json")) else {
        return false;
    };
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    for section in ["dependencies", "devDependencies"] {
        if let Some(map) = pkg.get(section).and_then(|v| v.as_object()) {
            if map.contains_key(name) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_utils::env_at;

    #[test]
    fn agent_table_has_key_agents() {
        assert!(get_agent("claude-code").is_some());
        assert!(get_agent("codex").is_some());
        assert!(get_agent("universal").is_some());
        assert!(get_agent("amp").is_some());
        assert!(get_agent("nope").is_none());
    }

    #[test]
    fn universal_agents_use_agents_skills_dir() {
        for a in ["amp", "codex", "cursor", "opencode"] {
            let agent = get_agent(a).unwrap();
            assert!(agent.is_universal(), "{a} should be universal");
        }
        let claude = get_agent("claude-code").unwrap();
        assert!(!claude.is_universal());
        assert_eq!(claude.skills_dir, ".claude/skills");
    }

    #[test]
    fn hidden_agents_excluded_from_universal_list() {
        let names: Vec<&str> = universal_agents().iter().map(|a| a.name).collect();
        assert!(!names.contains(&"dexto"));
        assert!(!names.contains(&"universal"));
        assert!(names.contains(&"amp"));
    }

    #[test]
    fn non_universal_agents_need_symlinks() {
        let names: Vec<&str> = non_universal_agents().iter().map(|a| a.name).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"windsurf"));
        assert!(!names.contains(&"amp"));
    }

    #[test]
    fn global_dir_resolution() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);

        // home base
        let cline = get_agent("cline").unwrap();
        assert_eq!(
            global_skills_dir(cline, &env).unwrap(),
            tmp.path().join(".agents/skills")
        );
        // config base
        let amp = get_agent("amp").unwrap();
        assert_eq!(
            global_skills_dir(amp, &env).unwrap(),
            tmp.path().join("config/agents/skills")
        );
        // no global
        let eve = get_agent("eve").unwrap();
        assert!(global_skills_dir(eve, &env).is_none());
    }

    #[test]
    fn detect_home_config_cwd() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);

        std::fs::create_dir_all(tmp.path().join(".cline")).unwrap();
        assert!(is_installed(get_agent("cline").unwrap(), &env));

        std::fs::create_dir_all(tmp.path().join("config/amp")).unwrap();
        assert!(is_installed(get_agent("amp").unwrap(), &env));

        std::fs::create_dir_all(tmp.path().join(".replit")).unwrap();
        assert!(is_installed(get_agent("replit").unwrap(), &env));
    }

    #[test]
    fn detect_home_or_cwd() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        std::fs::create_dir_all(tmp.path().join(".codebuddy")).unwrap();
        assert!(is_installed(get_agent("codebuddy").unwrap(), &env));
    }

    #[test]
    fn universal_never_detected_installed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        assert!(!is_installed(get_agent("universal").unwrap(), &env));
    }

    #[test]
    fn eve_detection_requires_package_dependency() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        std::fs::create_dir_all(tmp.path().join("agent")).unwrap();
        // no package.json → not installed
        assert!(!is_installed(get_agent("eve").unwrap(), &env));

        std::fs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"eve":"1.0.0"}}"#,
        )
        .unwrap();
        assert!(is_installed(get_agent("eve").unwrap(), &env));
    }
}
