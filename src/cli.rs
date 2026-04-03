//! CLI 入口与子命令定义 — 使用 clap derive 实现

use crate::config::ConfigManager;
use crate::console_ui::{self as ui, Spinner};
use crate::git::GitManager;
use crate::metadata::{self, SkillInfo};
use crate::platforms::PlatformRegistry;
use crate::skills::SkillManager;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "skill-repo",
    about = "Skill 仓库 CLI 工具 — 管理和共享 code agent 技能",
    version = VERSION,
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
pub enum Platform {
    Claude,
    Codex,
    Kiro,
}

impl Platform {
    fn as_str(&self) -> &str {
        match self {
            Platform::Claude => "claude",
            Platform::Codex => "codex",
            Platform::Kiro => "kiro",
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// 连接到远程 skill 仓库
    Connect {
        git_url: String,
        #[arg(long)]
        alias: Option<String>,
    },
    /// 初始化远程仓库为 skill 仓库
    Init {
        git_url: String,
        #[arg(long)]
        alias: Option<String>,
    },
    /// 查看仓库连接状态和 skill 概览
    Status,
    /// 从远程仓库安装 skill 到本地平台
    Install {
        #[arg(long, value_enum)]
        target: Platform,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        list: bool,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 上传本地 skill 到远程仓库
    Upload {
        #[arg(long, value_enum)]
        source: Platform,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        no_push: bool,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        list: bool,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 搜索仓库中的 skill
    Search {
        keyword: String,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 更新本地已安装的 skill
    Update {
        #[arg(long, value_enum)]
        target: Platform,
        #[arg(long)]
        dry_run: bool,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 从本地平台卸载 skill
    Remove {
        #[arg(long, value_enum)]
        target: Platform,
        #[arg(long)]
        skill: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// 对比本地 vs 远程 skill 差异
    Diff {
        #[arg(long)]
        skill: String,
        #[arg(long, value_enum)]
        target: Platform,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 脚手架创建新 skill
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "")]
        author: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long, value_enum)]
        target: Option<Platform>,
    },
    /// 查看 skill 的 Git 变更历史
    History {
        #[arg(long)]
        skill: String,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 将仓库中的 skill 回退到指定版本
    Rollback {
        #[arg(long)]
        skill: String,
        #[arg(long = "to")]
        commit_hash: String,
        #[arg(long)]
        push: bool,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 安装指定 Git 版本的 skill
    Pin {
        #[arg(long)]
        skill: String,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long, value_enum)]
        target: Platform,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 分支协作管理
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },
    /// 进入交互式 TUI 模式
    Interactive,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// 显示所有配置项
    Show,
    /// 更新配置项
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum BranchAction {
    /// 查看待合并的 skill 分支
    List {
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 合并分支到 main
    Merge {
        branch_name: String,
        #[arg(long = "from")]
        from_alias: Option<String>,
    },
    /// 切换协作模式
    Mode {
        #[arg(value_enum)]
        mode: BranchMode,
    },
}

#[derive(Clone, ValueEnum)]
enum BranchMode {
    Direct,
    Branch,
}

// ── helpers ──────────────────────────────────────────────────

fn get_config() -> ConfigManager {
    ConfigManager::new(None)
}

fn get_git() -> Result<GitManager> {
    let config = get_config();
    let cache_base = config
        .config_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("cache");
    Ok(GitManager::new(cache_base))
}

fn get_skill_manager(repo_path: Option<&Path>) -> SkillManager {
    let commands_dir = repo_path.map(|p| p.join("commands"));
    SkillManager::new(commands_dir)
}

fn require_connected(from_alias: Option<&str>) -> Result<(ConfigManager, String, PathBuf)> {
    let config = get_config();

    if let Some(alias) = from_alias {
        match config.get_repo(alias)? {
            Some(info) => return Ok((config, info.url, PathBuf::from(info.cache_path))),
            None => {
                let repos = config.get_repos()?;
                let hint = if repos.is_empty() {
                    "使用 skill-repo connect <git-url> --alias <name> 连接仓库".to_string()
                } else {
                    format!("可用仓库: {}", repos.keys().cloned().collect::<Vec<_>>().join(", "))
                };
                ui::error(&format!("未找到别名为 '{alias}' 的仓库。"), Some(&hint));
                bail!("仓库未找到");
            }
        }
    }

    let repo_url = config.get("repo.url")?;
    let cache_path = config.get("repo.cache_path")?;

    match (repo_url, cache_path) {
        (Some(url), Some(cp)) if !url.is_empty() && !cp.is_empty() => {
            Ok((config, url, PathBuf::from(cp)))
        }
        _ => {
            ui::error(
                "未连接到任何远程仓库。",
                Some("使用 skill-repo connect <git-url> 或 skill-repo init <git-url>"),
            );
            bail!("未连接仓库");
        }
    }
}

fn get_branch_mode() -> String {
    let config = get_config();
    config
        .get("branch.mode")
        .ok()
        .flatten()
        .filter(|m| m == "direct" || m == "branch")
        .unwrap_or_else(|| "direct".to_string())
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Connect { git_url, alias } => cmd_connect(&git_url, alias.as_deref()),
            Commands::Init { git_url, alias } => cmd_init(&git_url, alias.as_deref()),
            Commands::Status => cmd_status(),
            Commands::Install { target, skill, all, list, from_alias } => {
                cmd_install(target.as_str(), skill.as_deref(), all, list, from_alias.as_deref())
            }
            Commands::Upload { source, skill, no_push, category, list, from_alias } => {
                cmd_upload(source.as_str(), skill.as_deref(), no_push, category.as_deref(), list, from_alias.as_deref())
            }
            Commands::Search { keyword, from_alias } => {
                cmd_search(&keyword, from_alias.as_deref())
            }
            Commands::Update { target, dry_run, from_alias } => {
                cmd_update(target.as_str(), dry_run, from_alias.as_deref())
            }
            Commands::Remove { target, skill, yes } => {
                cmd_remove(target.as_str(), &skill, yes)
            }
            Commands::Diff { skill, target, from_alias } => {
                cmd_diff(&skill, target.as_str(), from_alias.as_deref())
            }
            Commands::Create { name, description, author, version, target } => {
                cmd_create(&name, &description, &author, &version, target.as_ref())
            }
            Commands::History { skill, limit, from_alias } => {
                cmd_history(&skill, limit, from_alias.as_deref())
            }
            Commands::Rollback { skill, commit_hash, push, from_alias } => {
                cmd_rollback(&skill, &commit_hash, push, from_alias.as_deref())
            }
            Commands::Pin { skill, commit, target, from_alias } => {
                cmd_pin(&skill, commit.as_deref(), target.as_str(), from_alias.as_deref())
            }
            Commands::Config { action } => match action {
                ConfigAction::Show => cmd_config_show(),
                ConfigAction::Set { key, value } => cmd_config_set(&key, &value),
            },
            Commands::Branch { action } => match action {
                BranchAction::List { from_alias } => cmd_branch_list(from_alias.as_deref()),
                BranchAction::Merge { branch_name, from_alias } => {
                    cmd_branch_merge(&branch_name, from_alias.as_deref())
                }
                BranchAction::Mode { mode } => {
                    let m = match mode {
                        BranchMode::Direct => "direct",
                        BranchMode::Branch => "branch",
                    };
                    cmd_config_set("branch.mode", m)
                }
            },
            Commands::Interactive => crate::interactive::run_interactive(),
        }
    }
}

// ── 命令实现 ─────────────────────────────────────────────────

fn cmd_connect(git_url: &str, alias: Option<&str>) -> Result<()> {
    let git = get_git()?;
    if !GitManager::validate_url(git_url) {
        ui::error(
            &format!("无效的 Git URL: {git_url}"),
            Some("支持: https://github.com/user/repo.git 或 git@github.com:user/repo.git"),
        );
        bail!("无效 URL");
    }
    let sp = Spinner::new(&format!("正在连接 {git_url} ..."));
    let repo_path = git.clone(git_url)?;
    sp.finish();

    if !GitManager::has_skills_dir(&repo_path) {
        ui::warning("该仓库不包含 skills/ 目录，可能不是有效的 skill 仓库。");
        ui::info("使用 skill-repo init <git-url> 初始化仓库结构。");
    }
    let config = get_config();
    let a = alias.unwrap_or("default");
    config.add_repo(a, git_url, &repo_path.to_string_lossy())?;
    ui::success(&format!("已成功连接到远程仓库 (别名: {a})。"));
    Ok(())
}

fn cmd_init(git_url: &str, alias: Option<&str>) -> Result<()> {
    let git = get_git()?;
    if !GitManager::validate_url(git_url) {
        ui::error(&format!("无效的 Git URL: {git_url}"), Some("支持 https:// 或 git@"));
        bail!("无效 URL");
    }
    let sp = Spinner::new(&format!("正在克隆 {git_url} ..."));
    let repo_path = git.clone(git_url)?;
    sp.finish();

    if GitManager::has_skills_dir(&repo_path) {
        ui::warning("该仓库已包含 skills/ 目录结构。");
        ui::info("建议使用 skill-repo connect <git-url> 直接连接。");
        return Ok(());
    }

    let sp = Spinner::new("正在创建标准 skill 仓库结构 ...");
    GitManager::init_repo_structure(&repo_path)?;
    sp.finish();

    let sp = Spinner::new("正在提交并推送初始结构 ...");
    match git.add_commit_push(&repo_path, "初始化 skill 仓库结构", true) {
        Ok(_) => {}
        Err(e) => {
            ui::warning(&format!("推送失败: {e}"));
            ui::info(&format!("本地结构已创建，请手动推送:\n    cd {}\n    git push", repo_path.display()));
        }
    }
    sp.finish();

    let config = get_config();
    let a = alias.unwrap_or("default");
    config.add_repo(a, git_url, &repo_path.to_string_lossy())?;
    ui::success("skill 仓库初始化完成。");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let config = get_config();
    let all_repos = config.get_repos()?;

    if all_repos.is_empty() {
        ui::warning("未连接到任何远程仓库");
        ui::info("使用 skill-repo connect <git-url> 连接已有仓库");
        ui::info("使用 skill-repo init <git-url> 初始化新仓库");
        return Ok(());
    }

    let current_url = config.get("repo.url")?.unwrap_or_default();
    let current_alias = all_repos
        .iter()
        .find(|(_, r)| r.url == current_url)
        .map(|(a, _)| a.as_str());

    if all_repos.len() > 1 {
        ui::print_repos_table(&all_repos, current_alias);
    }

    for (alias, repo_info) in &all_repos {
        let cache_path = PathBuf::from(&repo_info.cache_path);
        println!();
        if all_repos.len() > 1 {
            println!("  别名: {alias}");
        }
        println!("  URL: {}", repo_info.url);

        if cache_path.is_dir() {
            let sm = get_skill_manager(Some(&cache_path));
            let skills = sm.discover_skills(&cache_path.join("skills"));
            println!("  Skill 总数: {}", skills.len());
            if !skills.is_empty() {
                let mut cats: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
                for s in &skills {
                    *cats.entry(&s.category).or_default() += 1;
                }
                let cats_str: Vec<_> = cats.iter().map(|(c, n)| format!("{c}: {n}")).collect();
                println!("  分类: {}", cats_str.join("  "));
            }
        } else {
            println!("  Skill 总数: (缓存不可用)");
        }
    }

    // 本地平台
    println!();
    let registry = PlatformRegistry::new();
    for pc in registry.all() {
        let exists = pc.skills_dir.is_dir();
        let count = if exists {
            std::fs::read_dir(&pc.skills_dir)
                .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).count())
                .unwrap_or(0)
        } else {
            0
        };
        let status = if exists { "已安装" } else { "未安装" };
        println!("  {:<14} {:<8} {} 个 skill", pc.label, status, count);
    }
    println!();
    Ok(())
}

fn cmd_search(keyword: &str, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let sm = get_skill_manager(Some(&cache_path));
    let available = sm.discover_skills(&cache_path.join("skills"));
    let matched = sm.search_skills(&available, keyword);

    if matched.is_empty() {
        ui::info(&format!("未找到匹配 '{keyword}' 的 skill。"));
        return Ok(());
    }

    let owned: Vec<_> = matched.into_iter().cloned().collect();
    ui::print_skill_table(&owned, &format!("搜索结果: '{keyword}' ({} 个)", owned.len()));
    Ok(())
}

fn cmd_install(target: &str, skill: Option<&str>, all: bool, list: bool, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let sm = get_skill_manager(Some(&cache_path));
    let available = sm.discover_skills(&cache_path.join("skills"));

    if list || (skill.is_none() && !all) {
        if available.is_empty() {
            ui::warning("仓库中暂无 skill");
            return Ok(());
        }
        ui::print_skill_table(&available, "可用 Skill");
        return Ok(());
    }

    if all {
        let sp = Spinner::new(&format!("正在安装 {} 个 skill 到 {target} ...", available.len()));
        let count = sm.install_all(&cache_path.join("skills"), target)?;
        sp.finish();
        ui::success(&format!("已安装 {count} 个 skill 到 {target} 平台。"));
        return Ok(());
    }

    if let Some(name) = skill {
        let matched: Vec<_> = available.iter().filter(|s| s.metadata.name == name).collect();
        if matched.is_empty() {
            ui::error(&format!("未找到名为 '{name}' 的 skill。"), Some("使用 --list 查看可用 skill。"));
            bail!("skill 未找到");
        }
        let sp = Spinner::new(&format!("正在安装 {name} 到 {target} ..."));
        sm.install_skill(matched[0], target)?;
        sp.finish();
        ui::success(&format!("已安装 skill '{name}' 到 {target} 平台。"));
    }
    Ok(())
}

fn cmd_upload(
    source: &str, skill: Option<&str>, no_push: bool,
    category: Option<&str>, list: bool, from_alias: Option<&str>,
) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let registry = PlatformRegistry::new();
    let platform = registry.get(source)?;

    // 扫描本地 skill
    let mut local_skills = Vec::new();
    if platform.skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&platform.skills_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    let meta = metadata::parse_skill_md(&p.join("SKILL.md")).unwrap_or_default();
                    local_skills.push(SkillInfo {
                        metadata: meta,
                        category: "local".into(),
                        source_path: p,
                    });
                }
            }
        }
    }
    local_skills.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    if list {
        if local_skills.is_empty() {
            ui::warning(&format!("{} 平台暂无 skill", platform.label));
            return Ok(());
        }
        ui::print_skill_table(&local_skills, &format!("{} 本地 Skill", platform.label));
        return Ok(());
    }

    let skill_name = match skill {
        Some(n) => n,
        None => {
            ui::error("请通过 --skill 指定要上传的 skill 名称。", Some("使用 --list 查看本地可用 skill。"));
            bail!("未指定 skill");
        }
    };

    let matched: Vec<_> = local_skills.iter().filter(|s| s.metadata.name == skill_name).collect();
    if matched.is_empty() {
        ui::error(
            &format!("在 {} 平台未找到名为 '{skill_name}' 的 skill。", platform.label),
            Some("使用 --list 查看本地可用 skill。"),
        );
        bail!("skill 未找到");
    }

    let source_skill = matched[0];
    let errors = metadata::validate_skill(&source_skill.source_path);
    if !errors.is_empty() {
        ui::error(&format!("skill '{skill_name}' 元数据不完整:"), None);
        for e in &errors {
            eprintln!("    • {e}");
        }
        bail!("元数据不完整");
    }

    let cat = category.unwrap_or("uncategorized");
    let dest = cache_path.join("skills").join(cat).join(skill_name);
    let is_update = dest.exists();

    let sp = Spinner::new("正在复制 skill 到仓库缓存 ...");
    crate::skills::copy_skill(&source_skill.source_path, &dest)?;
    sp.finish();

    let action_label = if is_update { "更新" } else { "新增" };
    let commit_msg = GitManager::build_commit_message(
        action_label, skill_name, source, cat,
        &source_skill.metadata.description,
        &source_skill.metadata.version,
    );

    let git = get_git()?;
    let sm = get_skill_manager(Some(&cache_path));

    let sp = Spinner::new("正在提交到 Git ...");
    git.add_commit_push(&cache_path, &commit_msg, !no_push)?;
    sp.finish();

    // 同步
    let sp = Spinner::new("正在同步生成文件 ...");
    let sync = sm.sync_all(&cache_path)?;
    sp.finish();
    if sync.any_changed() {
        let _ = git.add_commit_push(&cache_path, "同步生成文件", !no_push);
    }

    ui::success(&format!("{action_label} skill '{skill_name}' → {cat} 分类"));
    Ok(())
}

fn cmd_update(target: &str, dry_run: bool, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let git = get_git()?;

    let sp = Spinner::new("正在拉取远程仓库最新内容 ...");
    if let Err(e) = git.pull(&cache_path) {
        ui::warning(&format!("拉取失败: {e}"));
    }
    sp.finish();

    let sm = get_skill_manager(Some(&cache_path));
    let (new, updated, unchanged) = sm.diff_skills(&cache_path.join("skills"), target)?;

    if new.is_empty() && updated.is_empty() {
        ui::success("所有已安装的 skill 均为最新。");
        if !unchanged.is_empty() {
            ui::info(&format!("{} 个 skill 无需更新。", unchanged.len()));
        }
        return Ok(());
    }

    ui::print_update_table(&new, &updated, &unchanged);

    if dry_run {
        ui::info(&format!("新增: {}  有更新: {}  最新: {}", new.len(), updated.len(), unchanged.len()));
        ui::info("使用不带 --dry-run 执行实际更新。");
        return Ok(());
    }

    if updated.is_empty() {
        ui::success("无需更新。");
        return Ok(());
    }

    let sp = Spinner::new(&format!("正在更新 {} 个 skill ...", updated.len()));
    for s in &updated {
        sm.install_skill(s, target)?;
    }
    sp.finish();

    ui::success(&format!("已更新 {} 个 skill 到 {target} 平台。", updated.len()));
    if !new.is_empty() {
        ui::info(&format!("另有 {} 个新 skill 可用，使用 skill-repo install 安装。", new.len()));
    }
    Ok(())
}

fn cmd_remove(target: &str, skill: &str, yes: bool) -> Result<()> {
    let sm = get_skill_manager(None);
    let platform = sm.platforms.get(target)?;
    let dest = platform.skills_dir.join(skill);

    if !dest.exists() && !dest.is_symlink() {
        ui::error(
            &format!("在 {target} 平台未找到名为 '{skill}' 的 skill。"),
            Some(&format!("使用 skill-repo install --target {target} --list 查看已安装 skill。")),
        );
        bail!("skill 未找到");
    }

    if !yes {
        print!("  确认卸载 '{skill}'? [y/N] ");
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  已取消");
            return Ok(());
        }
    }

    if sm.remove_skill(skill, target)? {
        ui::success(&format!("已从 {target} 平台卸载 skill '{skill}'。"));
    } else {
        ui::error("卸载失败。", None);
    }
    Ok(())
}

fn cmd_diff(skill: &str, target: &str, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let sm = get_skill_manager(Some(&cache_path));
    let registry = PlatformRegistry::new();
    let platform = registry.get(target)?;

    let local_dir = platform.skills_dir.join(skill);
    let available = sm.discover_skills(&cache_path.join("skills"));
    let remote = available.iter().find(|s| s.metadata.name == skill);

    if !local_dir.exists() && remote.is_none() {
        ui::error(&format!("skill '{skill}' 在本地和远程仓库中均不存在。"), None);
        bail!("skill 不存在");
    }
    if !local_dir.exists() {
        ui::info(&format!("skill '{skill}' 本地未安装，远程仓库中存在。"));
        ui::info("使用 skill-repo install 安装。");
        return Ok(());
    }
    if remote.is_none() {
        ui::info(&format!("skill '{skill}' 仅存在于本地 {target} 平台。"));
        ui::info("使用 skill-repo upload 上传到仓库。");
        return Ok(());
    }

    // 简单对比：检查文件是否一致
    let remote_dir = &remote.unwrap().source_path;
    if crate::skills::dirs_equal_pub(&local_dir, remote_dir) {
        ui::success(&format!("skill '{skill}' 本地与远程内容一致。"));
    } else {
        ui::warning(&format!("skill '{skill}' 本地与远程内容不一致。"));
    }
    Ok(())
}

fn cmd_create(name: &str, description: &str, author: &str, version: &str, target: Option<&Platform>) -> Result<()> {
    let target_dir = if let Some(t) = target {
        let registry = PlatformRegistry::new();
        let pc = registry.get(t.as_str())?;
        pc.skills_dir.clone()
    } else {
        std::env::current_dir()?
    };

    if target_dir.join(name).exists() {
        ui::error(&format!("目录 '{}' 已存在。", target_dir.join(name).display()), None);
        bail!("目录已存在");
    }

    let skill_dir = SkillManager::create_skill(&target_dir, name, description, author, version)?;
    ui::success(&format!("已创建 skill '{name}' → {}", skill_dir.display()));
    ui::info("编辑 SKILL.md 添加 prompt 内容，然后使用 skill-repo upload 上传到仓库。");
    Ok(())
}

fn cmd_history(skill: &str, limit: usize, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;

    let skill_path = match GitManager::find_skill_path(&cache_path, skill) {
        Some(p) => p,
        None => {
            ui::error(&format!("在仓库中未找到名为 '{skill}' 的 skill。"), Some("使用 skill-repo install --list 查看可用 skill。"));
            bail!("skill 未找到");
        }
    };

    let commits = GitManager::skill_log(&cache_path, &skill_path, limit)?;
    if commits.is_empty() {
        ui::info(&format!("skill '{skill}' 暂无变更历史。"));
        return Ok(());
    }

    ui::print_history_table(&commits, &format!("'{skill}' 变更历史"));
    ui::info(&format!("共 {} 条记录 (路径: {skill_path})", commits.len()));
    ui::info("使用 skill-repo rollback --skill <name> --to <commit> 回退到指定版本。");
    Ok(())
}

fn cmd_rollback(skill: &str, commit_hash: &str, push: bool, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let git = get_git()?;

    let skill_path = match GitManager::find_skill_path(&cache_path, skill) {
        Some(p) => p,
        None => {
            ui::error(&format!("在仓库中未找到名为 '{skill}' 的 skill。"), None);
            bail!("skill 未找到");
        }
    };

    let commits = GitManager::skill_log(&cache_path, &skill_path, 100)?;
    let matched = commits.iter().find(|c| c.hash.starts_with(commit_hash) || c.short_hash == commit_hash);

    let matched = match matched {
        Some(c) => c,
        None => {
            ui::error(&format!("未找到 commit '{commit_hash}'。"), Some(&format!("使用 skill-repo history --skill {skill} 查看可用版本。")));
            bail!("commit 未找到");
        }
    };

    ui::info(&format!("将回退 '{skill}' 到: {} ({}) {}", matched.short_hash, matched.date, matched.message));

    let sp = Spinner::new(&format!("正在回退 '{skill}' 到 {} ...", matched.short_hash));
    GitManager::restore_skill(&cache_path, &skill_path, &matched.hash)?;
    sp.finish();

    ui::success(&format!("已将 '{skill}' 回退到 {}。", matched.short_hash));

    if push {
        let msg = GitManager::build_commit_message(
            "回退", skill, "", "", &format!("→ {} ({})", matched.short_hash, matched.message), "",
        );
        let sp = Spinner::new("正在提交并推送 ...");
        match git.add_commit_push(&cache_path, &msg, true) {
            Ok(_) => { sp.finish(); ui::success("已提交并推送到远程仓库。"); }
            Err(e) => { sp.finish(); ui::warning(&format!("推送失败: {e}")); ui::info("回退已在本地生效，请手动 git push。"); }
        }
    } else {
        ui::info("回退仅在本地缓存生效。使用 --push 提交到远程。");
    }
    Ok(())
}

fn cmd_pin(skill: &str, commit: Option<&str>, target: &str, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;

    let skill_path = match GitManager::find_skill_path(&cache_path, skill) {
        Some(p) => p,
        None => {
            ui::error(&format!("在仓库中未找到名为 '{skill}' 的 skill。"), None);
            bail!("skill 未找到");
        }
    };

    let sm = get_skill_manager(Some(&cache_path));

    if let Some(hash) = commit {
        let commits = GitManager::skill_log(&cache_path, &skill_path, 100)?;
        let matched = commits.iter().find(|c| c.hash.starts_with(hash) || c.short_hash == hash);
        let matched = match matched {
            Some(c) => c,
            None => {
                ui::error(&format!("未找到 commit '{hash}'。"), Some(&format!("使用 skill-repo history --skill {skill} 查看。")));
                bail!("commit 未找到");
            }
        };

        // 用 git archive 提取指定版本
        let tmp = tempfile::tempdir()?;
        let output = std::process::Command::new("git")
            .args(["archive", &matched.hash, "--", &skill_path])
            .current_dir(&cache_path)
            .output()?;

        if !output.status.success() {
            ui::error("提取历史版本失败", None);
            bail!("git archive 失败");
        }

        // 解压 tar
        let cursor = std::io::Cursor::new(output.stdout);
        let mut archive = tar::Archive::new(cursor);
        archive.unpack(tmp.path())?;

        let extracted = tmp.path().join(&skill_path);
        if !extracted.is_dir() {
            ui::error("提取的 skill 目录不存在。", None);
            bail!("提取失败");
        }

        let meta = metadata::parse_skill_md(&extracted.join("SKILL.md")).unwrap_or_default();
        let skill_info = SkillInfo {
            metadata: meta,
            category: "pinned".into(),
            source_path: extracted,
        };

        let sp = Spinner::new(&format!("正在安装 {skill}@{} 到 {target} ...", matched.short_hash));
        sm.install_skill(&skill_info, target)?;
        sp.finish();
        ui::success(&format!("已安装 '{skill}' @ {} 到 {target} 平台。", matched.short_hash));
    } else {
        // HEAD 版本
        let available = sm.discover_skills(&cache_path.join("skills"));
        let matched: Vec<_> = available.iter().filter(|s| s.metadata.name == skill).collect();
        if matched.is_empty() {
            ui::error(&format!("未找到名为 '{skill}' 的 skill。"), None);
            bail!("skill 未找到");
        }
        let sp = Spinner::new(&format!("正在安装 {skill} (HEAD) 到 {target} ..."));
        sm.install_skill(matched[0], target)?;
        sp.finish();
        ui::success(&format!("已安装 '{skill}' (HEAD) 到 {target} 平台。"));
    }
    Ok(())
}

fn cmd_config_show() -> Result<()> {
    let cm = get_config();
    let data = cm.load()?;
    if data.is_empty() {
        ui::info("暂无配置项");
        return Ok(());
    }

    fn flatten(table: &toml::Table, prefix: &str) -> Vec<(String, String)> {
        let mut items = Vec::new();
        for (k, v) in table {
            let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
            match v {
                toml::Value::Table(t) => items.extend(flatten(t, &key)),
                other => items.push((key, other.to_string())),
            }
        }
        items
    }

    println!();
    println!("  {:<32} {}", console::style("配置项").bold().cyan(), console::style("值").dim());
    println!("  {}", console::style("─".repeat(56)).dim());
    for (key, val) in flatten(&data, "") {
        println!("  {:<32} {}", console::style(key).cyan(), console::style(val).dim());
    }
    println!();
    Ok(())
}

fn cmd_config_set(key: &str, value: &str) -> Result<()> {
    let cm = get_config();
    cm.set(key, value)?;
    ui::success(&format!("已设置 {key} = {value}"));
    Ok(())
}

fn cmd_branch_list(from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;
    let branches = GitManager::list_skill_branches(&cache_path)?;

    if branches.is_empty() {
        ui::info("暂无待合并的 skill 分支。");
        return Ok(());
    }

    println!();
    println!("  {}", console::style("待合并分支").bold().cyan());
    println!("  {:<40} {:<12} {}", console::style("分支名").bold(), console::style("日期").dim(), console::style("说明"));
    println!("  {}", console::style("─".repeat(68)).dim());
    for b in &branches {
        println!("  {:<40} {:<12} {}", console::style(&b.name).bold(), console::style(&b.last_date).dim(), b.last_commit);
    }
    println!();
    Ok(())
}

fn cmd_branch_merge(branch_name: &str, from_alias: Option<&str>) -> Result<()> {
    let (_config, _url, cache_path) = require_connected(from_alias)?;

    let sp = Spinner::new(&format!("正在合并 {branch_name} ..."));
    let merged = GitManager::try_merge_to_main(&cache_path, branch_name)?;
    sp.finish();

    if merged {
        let sp = Spinner::new("正在推送 ...");
        GitManager::push_main(&cache_path)?;
        sp.finish();
        ui::success(&format!("已合并 {branch_name} 到主分支。"));
    } else {
        ui::warning(&format!("无法自动合并 {branch_name}，存在冲突。"));
        ui::info("请手动解决冲突后合并。");
    }
    Ok(())
}
