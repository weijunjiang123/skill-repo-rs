//! Git 管理器 — 封装 git 命令行操作

use anyhow::{bail, Context, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git commit 信息
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// 分支信息
#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub last_commit: String,
    pub last_date: String,
}

pub struct GitManager {
    pub cache_dir: PathBuf,
}

impl GitManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// 验证 Git URL 格式
    pub fn validate_url(url: &str) -> bool {
        let https = Regex::new(r"^https://[^/]+/[^/]+/[^/]+(\.git)?$").unwrap();
        let ssh = Regex::new(r"^git@[^:]+:[^/]+/[^/]+(\.git)?$").unwrap();
        https.is_match(url) || ssh.is_match(url)
    }

    /// 根据 URL 生成确定性缓存路径
    pub fn get_cache_path(&self, url: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.cache_dir.join(&hash[..8])
    }

    /// 克隆远程仓库到本地缓存
    pub fn clone(&self, url: &str) -> Result<PathBuf> {
        let dest = self.get_cache_path(url);
        if dest.exists() {
            if self.has_commits(&dest) {
                self.pull(&dest)?;
            }
            return Ok(dest);
        }
        std::fs::create_dir_all(&self.cache_dir)?;
        run_git(&["clone", url, &dest.to_string_lossy()], None)?;
        Ok(dest)
    }

    /// 拉取最新内容
    pub fn pull(&self, repo_path: &Path) -> Result<()> {
        if !self.has_commits(repo_path) {
            return Ok(());
        }
        match run_git(&["pull"], Some(repo_path)) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("no such ref") || msg.contains("couldn't find remote ref") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    fn has_commits(&self, repo_path: &Path) -> bool {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// git add . → commit → optional push
    pub fn add_commit_push(&self, repo_path: &Path, message: &str, push: bool) -> Result<()> {
        run_git(&["add", "."], Some(repo_path))?;

        // 检查是否有变更
        let diff = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo_path)
            .output()?;

        if diff.status.success() {
            let status = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(repo_path)
                .output()?;
            if String::from_utf8_lossy(&status.stdout).trim().is_empty() {
                return Ok(()); // 无变更
            }
        }

        run_git(&["commit", "-m", message], Some(repo_path))?;

        if push {
            let branch = self.get_current_branch(repo_path);
            // 尝试直接 push，失败则 push -u
            match run_git(&["push"], Some(repo_path)) {
                Ok(_) => {}
                Err(_) => {
                    run_git(&["push", "-u", "origin", &branch], Some(repo_path))?;
                }
            }
        }
        Ok(())
    }

    /// 检查仓库是否包含 skills/ 目录
    pub fn has_skills_dir(repo_path: &Path) -> bool {
        repo_path.join("skills").is_dir()
    }

    /// 初始化仓库结构
    pub fn init_repo_structure(repo_path: &Path) -> Result<()> {
        use crate::templates;

        let write_if_missing = |path: PathBuf, content: &str| -> Result<()> {
            if !path.exists() {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, content)?;
            }
            Ok(())
        };

        write_if_missing(repo_path.join("README.md"), templates::ROOT_README)?;
        write_if_missing(repo_path.join(".gitignore"), templates::GITIGNORE)?;
        write_if_missing(repo_path.join("pyproject.toml"), templates::PYPROJECT_TOML)?;
        write_if_missing(repo_path.join("prek.toml"), templates::PREK_TOML)?;
        std::fs::create_dir_all(repo_path.join("skills"))?;
        write_if_missing(repo_path.join("skills/README.md"), templates::SKILLS_README)?;
        std::fs::create_dir_all(repo_path.join("commands"))?;
        write_if_missing(
            repo_path.join(".claude-plugin/manifest.json"),
            templates::MANIFEST_JSON,
        )?;
        let sd = repo_path.join("scripts");
        std::fs::create_dir_all(&sd)?;
        write_if_missing(sd.join("post_commit_sync.py"), templates::POST_COMMIT_SYNC)?;
        write_if_missing(sd.join("sync_commands.py"), templates::SYNC_COMMANDS)?;
        write_if_missing(sd.join("sync_skills_readme.py"), templates::SYNC_SKILLS_README)?;
        write_if_missing(
            sd.join("sync_claude_marketplace.py"),
            templates::SYNC_CLAUDE_MARKETPLACE,
        )?;
        write_if_missing(
            repo_path.join("skills/tools/skill-repo-cli/SKILL.md"),
            templates::DEFAULT_SKILL_MD,
        )?;

        Ok(())
    }

    /// 构建规范化 commit message
    pub fn build_commit_message(
        action: &str,
        skill_name: &str,
        source: &str,
        category: &str,
        description: &str,
        version: &str,
    ) -> String {
        let emoji = match action {
            "新增" => "✨",
            "更新" => "📦",
            "回退" => "⏪",
            "删除" => "🗑️",
            _ => "📝",
        };

        let mut subject = format!("{emoji} [{action}] {skill_name}");
        if !description.is_empty() {
            let short: String = {
                let mut chars = description.chars();
                let truncated: String = chars.by_ref().take(50).collect();
                if chars.next().is_some() {
                    format!("{truncated}…")
                } else {
                    truncated
                }
            };
            subject.push_str(&format!(" — {short}"));
        }

        let mut meta = Vec::new();
        if !source.is_empty() {
            meta.push(format!("来源: {source}"));
        }
        if !category.is_empty() {
            meta.push(format!("分类: {category}"));
        }
        if !version.is_empty() {
            meta.push(format!("版本: {version}"));
        }

        if meta.is_empty() {
            subject
        } else {
            format!("{subject}\n\n{}", meta.join(" | "))
        }
    }

    /// 查找 skill 在仓库中的相对路径
    pub fn find_skill_path(repo_path: &Path, skill_name: &str) -> Option<String> {
        let skills_dir = repo_path.join("skills");
        if !skills_dir.is_dir() {
            return None;
        }

        for entry in walkdir::WalkDir::new(&skills_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "SKILL.md" {
                let parent = entry.path().parent()?;
                if parent.file_name()?.to_str()? == skill_name {
                    return Some(
                        parent
                            .strip_prefix(repo_path)
                            .ok()?
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }

        // 也尝试匹配 frontmatter 中的 name
        for entry in walkdir::WalkDir::new(&skills_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "SKILL.md" {
                if let Ok(meta) = crate::metadata::parse_skill_md(entry.path()) {
                    if meta.name == skill_name {
                        let parent = entry.path().parent()?;
                        return Some(
                            parent
                                .strip_prefix(repo_path)
                                .ok()?
                                .to_string_lossy()
                                .replace('\\', "/"),
                        );
                    }
                }
            }
        }

        None
    }

    /// 获取 skill 的 git log 历史
    pub fn skill_log(
        repo_path: &Path,
        skill_rel_path: &str,
        max_count: usize,
    ) -> Result<Vec<CommitInfo>> {
        let fmt = "%H%n%h%n%an%n%ai%n%s";
        let output = run_git(
            &[
                "log",
                &format!("--max-count={max_count}"),
                &format!("--format={fmt}"),
                "--",
                skill_rel_path,
            ],
            Some(repo_path),
        )?;

        let lines: Vec<&str> = output.trim().split('\n').collect();
        let mut commits = Vec::new();
        let mut i = 0;
        while i + 4 < lines.len() {
            commits.push(CommitInfo {
                hash: lines[i].to_string(),
                short_hash: lines[i + 1].to_string(),
                author: lines[i + 2].to_string(),
                date: lines[i + 3].chars().take(10).collect(),
                message: lines[i + 4].to_string(),
            });
            i += 5;
        }
        Ok(commits)
    }

    /// 将 skill 恢复到指定 commit
    pub fn restore_skill(
        repo_path: &Path,
        skill_rel_path: &str,
        commit_hash: &str,
    ) -> Result<()> {
        run_git(
            &["checkout", commit_hash, "--", skill_rel_path],
            Some(repo_path),
        )?;
        Ok(())
    }

    // ── 分支协作 ──────────────────────────────────────────────

    /// 获取 git 用户名（kebab-case）
    pub fn get_username(repo_path: &Path) -> String {
        let name = run_git(&["config", "user.name"], Some(repo_path))
            .unwrap_or_default()
            .trim()
            .to_string();

        let name = if name.is_empty() {
            whoami::username()
        } else {
            name
        };

        let name = name.to_lowercase();
        let re = Regex::new(r"[\s_]+").unwrap();
        let name = re.replace_all(&name, "-");
        let re2 = Regex::new(r"[^a-z0-9\-]").unwrap();
        let name = re2.replace_all(&name, "");
        if name.is_empty() {
            "anonymous".to_string()
        } else {
            name.to_string()
        }
    }

    /// 创建 skill 分支
    pub fn create_skill_branch(
        &self,
        repo_path: &Path,
        username: &str,
        action: &str,
        skill_name: &str,
    ) -> Result<String> {
        let main = get_main_branch(repo_path);
        run_git(&["checkout", &main], Some(repo_path))?;
        let _ = self.pull(repo_path);

        let branch = format!("skill/{username}/{action}-{skill_name}");
        let _ = run_git(&["branch", "-D", &branch], Some(repo_path));
        run_git(&["checkout", "-b", &branch], Some(repo_path))?;
        Ok(branch)
    }

    /// 推送分支
    pub fn push_branch(repo_path: &Path, branch: &str) -> Result<()> {
        run_git(&["push", "-u", "origin", branch], Some(repo_path))?;
        Ok(())
    }

    /// 尝试合并到 main
    pub fn try_merge_to_main(repo_path: &Path, branch: &str) -> Result<bool> {
        let main = get_main_branch(repo_path);
        run_git(&["checkout", &main], Some(repo_path))?;
        let _ = run_git(&["pull"], Some(repo_path));

        // 先尝试 ff-only
        if run_git(&["merge", "--ff-only", branch], Some(repo_path)).is_ok() {
            return Ok(true);
        }

        // 尝试普通 merge
        match run_git(
            &["merge", branch, "-m", &format!("合并分支: {branch}")],
            Some(repo_path),
        ) {
            Ok(_) => Ok(true),
            Err(_) => {
                let _ = run_git(&["merge", "--abort"], Some(repo_path));
                Ok(false)
            }
        }
    }

    /// 推送 main
    pub fn push_main(repo_path: &Path) -> Result<()> {
        let main = get_main_branch(repo_path);
        run_git(&["push", "origin", &main], Some(repo_path))?;
        Ok(())
    }

    /// 删除远程分支
    pub fn delete_remote_branch(repo_path: &Path, branch: &str) {
        let _ = run_git(&["push", "origin", "--delete", branch], Some(repo_path));
        let _ = run_git(&["branch", "-d", branch], Some(repo_path));
    }

    /// 列出 skill/ 开头的远程分支
    pub fn list_skill_branches(repo_path: &Path) -> Result<Vec<BranchInfo>> {
        let _ = run_git(&["fetch", "--prune"], Some(repo_path));

        let output = run_git(
            &[
                "branch",
                "-r",
                "--format=%(refname:short) %(committerdate:short) %(subject)",
            ],
            Some(repo_path),
        )
        .unwrap_or_default();

        let mut branches = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if !line.starts_with("origin/skill/") {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            let name = parts[0].strip_prefix("origin/").unwrap_or(parts[0]);
            branches.push(BranchInfo {
                name: name.to_string(),
                is_remote: true,
                last_commit: parts.get(2).unwrap_or(&"").to_string(),
                last_date: parts.get(1).unwrap_or(&"").to_string(),
            });
        }
        Ok(branches)
    }

    fn get_current_branch(&self, repo_path: &Path) -> String {
        run_git(&["branch", "--show-current"], Some(repo_path))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "main".to_string())
    }
}

/// 获取主分支名
fn get_main_branch(repo_path: &Path) -> String {
    for name in ["main", "master"] {
        if run_git(&["rev-parse", "--verify", name], Some(repo_path)).is_ok() {
            return name.to_string();
        }
    }
    "main".to_string()
}

/// 执行 git 命令，返回 stdout
fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output().context("无法执行 git 命令")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git 命令失败: git {}\n{}", args.join(" "), stderr.trim());
    }
}
