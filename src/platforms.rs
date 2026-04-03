//! 平台路径注册表 — 管理各 code agent 平台的路径配置

use std::env;
use std::path::PathBuf;

/// 平台配置
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub name: String,
    pub label: String,
    pub skills_dir: PathBuf,
    pub has_commands: bool,
    pub commands_dir: Option<PathBuf>,
}

/// 平台注册表
pub struct PlatformRegistry {
    platforms: Vec<PlatformConfig>,
}

impl PlatformRegistry {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Claude
        let claude_base = env::var("CLAUDE_SKILLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".claude"));
        let claude = PlatformConfig {
            name: "claude".into(),
            label: "Claude Code".into(),
            skills_dir: claude_base.join("skills"),
            has_commands: true,
            commands_dir: Some(claude_base.join("commands")),
        };

        // Codex
        let codex_skills = env::var("CODEX_SKILLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".codex").join("skills"));
        let codex = PlatformConfig {
            name: "codex".into(),
            label: "Codex".into(),
            skills_dir: codex_skills,
            has_commands: false,
            commands_dir: None,
        };

        // Kiro
        let kiro_skills = env::var("KIRO_SKILLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".kiro").join("skills"));
        let kiro = PlatformConfig {
            name: "kiro".into(),
            label: "Kiro".into(),
            skills_dir: kiro_skills,
            has_commands: false,
            commands_dir: None,
        };

        Self {
            platforms: vec![claude, codex, kiro],
        }
    }

    pub fn get(&self, name: &str) -> anyhow::Result<&PlatformConfig> {
        self.platforms
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| {
                let valid: Vec<_> = self.platforms.iter().map(|p| p.name.as_str()).collect();
                anyhow::anyhow!("未知平台 '{name}'，支持的平台: {}", valid.join(", "))
            })
    }

    pub fn all(&self) -> &[PlatformConfig] {
        &self.platforms
    }

    pub fn names(&self) -> Vec<&str> {
        self.platforms.iter().map(|p| p.name.as_str()).collect()
    }
}
