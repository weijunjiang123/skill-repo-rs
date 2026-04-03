//! Skill 管理器 — 发现、安装、搜索、同步

use crate::metadata::{self, SkillInfo, SkillMetadata};
use crate::platforms::PlatformRegistry;
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Skill 管理器
pub struct SkillManager {
    pub platforms: PlatformRegistry,
    pub commands_dir: Option<PathBuf>,
}

// ── 同步相关常量 ─────────────────────────────────────────────

const SKILLS_README_START: &str = "<!-- BEGIN AUTO SKILLS -->";
const SKILLS_README_END: &str = "<!-- END AUTO SKILLS -->";

impl SkillManager {
    pub fn new(commands_dir: Option<PathBuf>) -> Self {
        Self {
            platforms: PlatformRegistry::new(),
            commands_dir,
        }
    }

    /// 递归扫描目录发现所有 skill
    pub fn discover_skills(&self, skills_dir: &Path) -> Vec<SkillInfo> {
        let mut skills = Vec::new();
        if !skills_dir.is_dir() {
            return skills;
        }

        let mut entries: Vec<_> = WalkDir::new(skills_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "SKILL.md")
            .collect();
        entries.sort_by_key(|e| e.path().to_path_buf());

        for entry in entries {
            let rel = entry
                .path()
                .strip_prefix(skills_dir)
                .unwrap_or(entry.path());

            // 跳过以 _ 开头的私有目录
            if rel.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .map(|s| s.starts_with('_'))
                    .unwrap_or(false)
            }) {
                continue;
            }

            let skill_dir = entry.path().parent().unwrap_or(entry.path());
            let meta = metadata::parse_skill_md(entry.path()).unwrap_or_default();

            let rel_skill = skill_dir
                .strip_prefix(skills_dir)
                .unwrap_or(skill_dir);
            let category = if rel_skill.components().count() <= 1 {
                "uncategorized".to_string()
            } else {
                rel_skill
                    .components()
                    .next()
                    .and_then(|c| c.as_os_str().to_str())
                    .unwrap_or("uncategorized")
                    .to_string()
            };

            skills.push(SkillInfo {
                metadata: meta,
                category,
                source_path: skill_dir.to_path_buf(),
            });
        }
        skills
    }

    /// 安装 skill 到目标平台
    pub fn install_skill(&self, skill: &SkillInfo, target: &str) -> Result<()> {
        let platform = self.platforms.get(target)?;
        let dest = platform.skills_dir.join(&skill.metadata.name);
        copy_skill(&skill.source_path, &dest)?;

        // Claude: 同步 command 文件
        if platform.has_commands {
            if let Some(ref cmds_dir) = self.commands_dir {
                let cmd_src = cmds_dir.join(format!("{}.md", skill.metadata.name));
                if cmd_src.is_file() {
                    if let Some(ref cmd_dest_dir) = platform.commands_dir {
                        std::fs::create_dir_all(cmd_dest_dir)?;
                        std::fs::copy(&cmd_src, cmd_dest_dir.join(format!("{}.md", skill.metadata.name)))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// 安装所有 skill
    pub fn install_all(&self, skills_dir: &Path, target: &str) -> Result<usize> {
        let skills = self.discover_skills(skills_dir);
        for skill in &skills {
            self.install_skill(skill, target)?;
        }
        Ok(skills.len())
    }

    /// 搜索 skill
    pub fn search_skills<'a>(&self, skills: &'a [SkillInfo], keyword: &str) -> Vec<&'a SkillInfo> {
        let kw = keyword.to_lowercase();
        skills
            .iter()
            .filter(|s| {
                s.metadata.name.to_lowercase().contains(&kw)
                    || s.metadata.description.to_lowercase().contains(&kw)
                    || s.category.to_lowercase().contains(&kw)
            })
            .collect()
    }

    /// 从平台删除 skill
    pub fn remove_skill(&self, skill_name: &str, target: &str) -> Result<bool> {
        let platform = self.platforms.get(target)?;
        let dest = platform.skills_dir.join(skill_name);

        if !dest.exists() && !dest.is_symlink() {
            return Ok(false);
        }

        if dest.is_symlink() || dest.is_file() {
            std::fs::remove_file(&dest)?;
        } else if dest.is_dir() {
            std::fs::remove_dir_all(&dest)?;
        }

        // Claude: 删除 command 文件
        if platform.has_commands {
            if let Some(ref cmd_dir) = platform.commands_dir {
                let cmd_file = cmd_dir.join(format!("{skill_name}.md"));
                if cmd_file.exists() {
                    std::fs::remove_file(&cmd_file)?;
                }
            }
        }
        Ok(true)
    }

    /// 列出已安装的 skill
    pub fn list_installed(&self, target: &str) -> Result<Vec<SkillInfo>> {
        let platform = self.platforms.get(target)?;
        let mut skills = Vec::new();
        if !platform.skills_dir.is_dir() {
            return Ok(skills);
        }
        let mut entries: Vec<_> = std::fs::read_dir(&platform.skills_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let meta = metadata::parse_skill_md(&entry.path().join("SKILL.md"))
                .unwrap_or_default();
            skills.push(SkillInfo {
                metadata: meta,
                category: "installed".to_string(),
                source_path: entry.path(),
            });
        }
        Ok(skills)
    }

    /// 对比远程 vs 本地
    pub fn diff_skills(
        &self,
        skills_dir: &Path,
        target: &str,
    ) -> Result<(Vec<SkillInfo>, Vec<SkillInfo>, Vec<SkillInfo>)> {
        let remote = self.discover_skills(skills_dir);
        let installed = self.list_installed(target)?;
        let installed_map: std::collections::HashMap<_, _> = installed
            .iter()
            .map(|s| (s.metadata.name.clone(), s))
            .collect();

        let mut new = Vec::new();
        let mut updated = Vec::new();
        let mut unchanged = Vec::new();

        for rs in remote {
            match installed_map.get(&rs.metadata.name) {
                None => new.push(rs),
                Some(local) => {
                    if !dirs_equal(&rs.source_path, &local.source_path) {
                        updated.push(rs);
                    } else {
                        unchanged.push(rs);
                    }
                }
            }
        }
        Ok((new, updated, unchanged))
    }

    /// 创建新 skill 脚手架
    pub fn create_skill(
        target_dir: &Path,
        name: &str,
        description: &str,
        author: &str,
        version: &str,
    ) -> Result<PathBuf> {
        let skill_dir = target_dir.join(name);
        std::fs::create_dir_all(&skill_dir)?;

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let meta = SkillMetadata {
            name: name.to_string(),
            description: description.to_string(),
            version: version.to_string(),
            author: author.to_string(),
            updated: today,
        };

        let content = format!(
            "{}\n# {name}\n\n在此编写 skill 的详细说明和 prompt 内容...\n",
            metadata::format_frontmatter(&meta)
        );
        std::fs::write(skill_dir.join("SKILL.md"), content)?;
        Ok(skill_dir)
    }

    // ── 内置同步 ──────────────────────────────────────────────

    /// 运行所有同步任务
    pub fn sync_all(&self, repo_path: &Path) -> Result<SyncResult> {
        Ok(SyncResult {
            skills_readme: self.sync_skills_readme(repo_path)?,
            commands: self.sync_commands(repo_path)?,
            manifest: self.sync_manifest(repo_path)?,
        })
    }

    fn sync_skills_readme(&self, repo_path: &Path) -> Result<bool> {
        let skills_dir = repo_path.join("skills");
        let readme = skills_dir.join("README.md");
        let skills = self.discover_skills(&skills_dir);

        let mut lines = vec![
            SKILLS_README_START.to_string(),
            "| Skill | Description | Path |".to_string(),
            "| --- | --- | --- |".to_string(),
        ];
        for s in &skills {
            let rel = s
                .source_path
                .strip_prefix(repo_path)
                .unwrap_or(&s.source_path)
                .to_string_lossy()
                .replace('\\', "/");
            lines.push(format!(
                "| `{}` | {} | [`{rel}`](../{rel}/SKILL.md) |",
                s.metadata.name, s.metadata.description
            ));
        }
        lines.push(SKILLS_README_END.to_string());
        let generated = lines.join("\n");

        if !readme.exists() {
            std::fs::write(&readme, format!("{generated}\n"))?;
            return Ok(true);
        }

        let content = std::fs::read_to_string(&readme)?;
        let updated = if let (Some(start), Some(end)) =
            (content.find(SKILLS_README_START), content.find(SKILLS_README_END))
        {
            format!(
                "{}{}{}",
                &content[..start],
                generated,
                &content[end + SKILLS_README_END.len()..]
            )
        } else {
            format!("{}\n\n{generated}\n", content.trim_end())
        };

        if updated != content {
            std::fs::write(&readme, updated)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn sync_commands(&self, repo_path: &Path) -> Result<bool> {
        let skills_dir = repo_path.join("skills");
        let commands_dir = repo_path.join("commands");
        std::fs::create_dir_all(&commands_dir)?;

        let skills = self.discover_skills(&skills_dir);
        let mut changed = false;

        for s in &skills {
            if s.metadata.description.is_empty() {
                continue;
            }
            let desired = format!(
                "---\ndescription: {}\nlocation: plugin\n---\n\nUse the `{}` skill to help with this task.\n",
                s.metadata.description, s.metadata.name
            );
            let cmd_file = commands_dir.join(format!("{}.md", s.metadata.name));
            if cmd_file.exists() && std::fs::read_to_string(&cmd_file)? == desired {
                continue;
            }
            std::fs::write(&cmd_file, desired)?;
            changed = true;
        }
        Ok(changed)
    }

    fn sync_manifest(&self, repo_path: &Path) -> Result<bool> {
        let skills_dir = repo_path.join("skills");
        let manifest_path = repo_path.join(".claude-plugin/manifest.json");
        std::fs::create_dir_all(manifest_path.parent().unwrap())?;

        let skills = self.discover_skills(&skills_dir);
        let entries: Vec<serde_json::Value> = skills
            .iter()
            .map(|s| {
                let rel = s
                    .source_path
                    .strip_prefix(repo_path)
                    .unwrap_or(&s.source_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let mut entry = serde_json::json!({
                    "name": s.metadata.name,
                    "path": rel,
                    "command": format!("commands/{}.md", s.metadata.name),
                    "tested": false,
                });
                if s.category != "uncategorized" {
                    entry["category"] = serde_json::json!(s.category);
                }
                entry
            })
            .collect();

        let mut data: serde_json::Value = if manifest_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&manifest_path)?).unwrap_or_default()
        } else {
            serde_json::json!({})
        };

        if data.get("skills") == Some(&serde_json::json!(entries)) {
            return Ok(false);
        }

        data["skills"] = serde_json::json!(entries);
        if data.get("name").is_none() {
            data["name"] = serde_json::json!("skill-repo");
        }
        if data.get("version").is_none() {
            data["version"] = serde_json::json!("1.0.0");
        }

        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&data)? + "\n",
        )?;
        Ok(true)
    }
}

pub struct SyncResult {
    pub skills_readme: bool,
    pub commands: bool,
    pub manifest: bool,
}

impl SyncResult {
    pub fn any_changed(&self) -> bool {
        self.skills_readme || self.commands || self.manifest
    }
}

/// 复制 skill 目录
pub fn copy_skill(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let options = fs_extra::dir::CopyOptions::new();
    fs_extra::dir::copy(src, dest.parent().unwrap(), &options)?;

    // fs_extra copies as dest_parent/src_name, rename if needed
    let copied_name = src.file_name().unwrap();
    let actual = dest.parent().unwrap().join(copied_name);
    if actual != *dest && actual.exists() {
        if dest.exists() {
            std::fs::remove_dir_all(dest)?;
        }
        std::fs::rename(&actual, dest)?;
    }
    Ok(())
}

/// 公开的目录比较（供 cli diff 使用）
pub fn dirs_equal_pub(a: &Path, b: &Path) -> bool {
    dirs_equal(a, b)
}

/// 递归比较两个目录
fn dirs_equal(a: &Path, b: &Path) -> bool {
    if !a.is_dir() || !b.is_dir() {
        return false;
    }

    let a_files: std::collections::BTreeSet<_> = WalkDir::new(a)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(a).ok().map(|p| p.to_path_buf()))
        .collect();

    let b_files: std::collections::BTreeSet<_> = WalkDir::new(b)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(b).ok().map(|p| p.to_path_buf()))
        .collect();

    if a_files != b_files {
        return false;
    }

    a_files.iter().all(|rel| {
        let fa = a.join(rel);
        let fb = b.join(rel);
        match (std::fs::read(&fa), std::fs::read(&fb)) {
            (Ok(ca), Ok(cb)) => ca == cb,
            _ => false,
        }
    })
}
