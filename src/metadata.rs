//! SKILL.md frontmatter 解析器

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Skill 元数据（来自 SKILL.md 的 YAML frontmatter）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub updated: String,
}

/// Skill 完整信息
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub metadata: SkillMetadata,
    pub category: String,
    pub source_path: PathBuf,
}

/// 解析 SKILL.md 的 YAML frontmatter
pub fn parse_skill_md(path: &Path) -> Result<SkillMetadata> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取 {}", path.display()))?;

    let re = Regex::new(r"(?s)\A---\s*\n(.*?)\n---").unwrap();

    if let Some(caps) = re.captures(&content) {
        let yaml_str = &caps[1];
        let meta: SkillMetadata = serde_yaml::from_str(yaml_str).unwrap_or_else(|_| {
            let dir_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            SkillMetadata {
                name: dir_name.to_string(),
                ..Default::default()
            }
        });
        Ok(meta)
    } else {
        // 回退：用父目录名
        let dir_name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        Ok(SkillMetadata {
            name: dir_name.to_string(),
            ..Default::default()
        })
    }
}

/// 验证 skill 目录的元数据完整性
pub fn validate_skill(skill_dir: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let skill_md = skill_dir.join("SKILL.md");

    if !skill_md.exists() {
        errors.push("缺少 SKILL.md 文件".to_string());
        return errors;
    }

    match parse_skill_md(&skill_md) {
        Ok(meta) => {
            if meta.name.is_empty() {
                errors.push("name 字段为空".to_string());
            }
            if meta.description.is_empty() {
                errors.push("description 字段为空".to_string());
            }
        }
        Err(e) => errors.push(format!("解析失败: {e}")),
    }
    errors
}

/// 将元数据格式化为 YAML frontmatter
pub fn format_frontmatter(meta: &SkillMetadata) -> String {
    let mut lines = vec![
        "---".to_string(),
        format!("name: \"{}\"", meta.name),
        format!("description: \"{}\"", meta.description),
    ];
    if !meta.version.is_empty() {
        lines.push(format!("version: \"{}\"", meta.version));
    }
    if !meta.author.is_empty() {
        lines.push(format!("author: \"{}\"", meta.author));
    }
    if !meta.updated.is_empty() {
        lines.push(format!("updated: \"{}\"", meta.updated));
    }
    lines.push("---".to_string());
    lines.join("\n") + "\n"
}
