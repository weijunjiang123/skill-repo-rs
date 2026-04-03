//! 交互式 TUI 模式 — 使用 dialoguer 实现菜单式交互

use crate::config::ConfigManager;
use crate::console_ui::{self as ui, Spinner};
use crate::git::GitManager;
use crate::metadata::{self, SkillInfo};
use crate::platforms::PlatformRegistry;
use crate::skills::SkillManager;
use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use std::path::{Path, PathBuf};

fn get_config() -> ConfigManager {
    ConfigManager::new(None)
}

fn get_git() -> Result<GitManager> {
    let config = get_config();
    let cache_base = config.config_path.parent().unwrap_or(Path::new(".")).join("cache");
    Ok(GitManager::new(cache_base))
}

fn get_connected_repo() -> Option<(String, PathBuf)> {
    let config = get_config();
    let url = config.get("repo.url").ok()??;
    let cache = config.get("repo.cache_path").ok()??;
    if url.is_empty() || cache.is_empty() {
        return None;
    }
    let p = PathBuf::from(&cache);
    if p.is_dir() { Some((url, p)) } else { None }
}

/// UTF-8 安全截断
fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn pick_platform(prompt: &str) -> Option<String> {
    let registry = PlatformRegistry::new();
    let names: Vec<String> = registry.all().iter().map(|p| p.label.clone()).collect();
    let idx = Select::new().with_prompt(prompt).items(&names).default(0).interact_opt().ok()??;
    Some(registry.all()[idx].name.clone())
}

fn banner() {
    println!();
    println!("  {}", style("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━").cyan());
    println!("  {}  {}", style("Skill Repo").bold().cyan(), style("— 团队 Skill 共享管理工具").dim());
    println!("  {}", style("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━").cyan());
    println!();
}

fn pause() {
    println!();
    let _: String = Input::new()
        .with_prompt("  按 Enter 返回主菜单")
        .allow_empty(true)
        .default(String::new())
        .interact_text()
        .unwrap_or_default();
}

pub fn run_interactive() -> Result<()> {
    banner();

    let menu = vec![
        "📋  概览",
        "📥  安装 Skill",
        "📤  上传 Skill",
        "🔍  搜索 Skill",
        "🔄  更新 Skill",
        "🗑️   卸载 Skill",
        "📜  版本管理",
        "🔗  仓库管理",
        "⚙️   设置",
        "🚪  退出",
    ];

    loop {
        let selection = Select::new()
            .with_prompt("操作")
            .items(&menu)
            .default(0)
            .interact_opt()?;

        match selection {
            None | Some(9) => break,
            Some(0) => action_overview(),
            Some(1) => action_install(),
            Some(2) => action_upload(),
            Some(3) => action_search(),
            Some(4) => action_update(),
            Some(5) => action_remove(),
            Some(6) => action_version_mgmt(),
            Some(7) => action_repo(),
            Some(8) => action_settings(),
            _ => {}
        }
        pause();
        println!();
    }

    println!("\n  {} 👋\n", style("再见").dim());
    Ok(())
}

fn action_overview() {
    let conn = get_connected_repo();
    if let Some((url, cache_path)) = &conn {
        let sm = SkillManager::new(Some(cache_path.join("commands")));
        let skills = sm.discover_skills(&cache_path.join("skills"));
        println!();
        println!("  仓库: {url}");
        println!("  Skill 总数: {}", skills.len());
        if !skills.is_empty() {
            let mut cats: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
            for s in &skills { *cats.entry(&s.category).or_default() += 1; }
            let cats_str: Vec<_> = cats.iter().map(|(c, n)| format!("{c}: {n}")).collect();
            println!("  分类: {}", cats_str.join("  "));
        }
    } else {
        ui::warning("未连接到远程仓库");
        ui::info("使用「仓库管理」连接或初始化仓库");
    }

    println!();
    let registry = PlatformRegistry::new();
    for pc in registry.all() {
        if !pc.skills_dir.is_dir() {
            println!("  {}  {}", style(&pc.label).bold(), style("— 未安装").dim());
            continue;
        }
        let count = std::fs::read_dir(&pc.skills_dir)
            .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists()).count())
            .unwrap_or(0);
        println!("  {}  {} 个 skill", style(&pc.label).bold(), style(count).green());
    }
}

fn action_install() {
    let conn = match get_connected_repo() {
        Some(c) => c,
        None => { ui::warning("未连接到远程仓库"); return; }
    };
    let (_url, cache_path) = conn;
    let sm = SkillManager::new(Some(cache_path.join("commands")));
    let available = sm.discover_skills(&cache_path.join("skills"));

    if available.is_empty() {
        ui::warning("仓库中暂无 skill");
        return;
    }

    let platform = match pick_platform("安装到哪个平台?") {
        Some(p) => p,
        None => return,
    };

    ui::print_skill_table(&available, "可用 Skill");

    let names: Vec<String> = available.iter().map(|s| {
        let desc = truncate_str(&s.metadata.description, 38);
        format!("{} ({}) — {desc}", s.metadata.name, s.category)
    }).collect();

    let selected = MultiSelect::new()
        .with_prompt("选择要安装的 skill (Space 选择, Enter 确认)")
        .items(&names)
        .interact_opt()
        .unwrap_or(None);

    let selected = match selected {
        Some(s) if !s.is_empty() => s,
        _ => { println!("  已取消"); return; }
    };

    let sp = Spinner::new(&format!("正在安装 {} 个 skill ...", selected.len()));
    for &idx in &selected {
        if let Err(e) = sm.install_skill(&available[idx], &platform) {
            ui::error(&format!("安装 {} 失败: {e}", available[idx].metadata.name), None);
        }
    }
    sp.finish();

    for &idx in &selected {
        ui::success(&available[idx].metadata.name);
    }
    println!("  已安装 {} 个 skill 到 {platform}", selected.len());
}

fn action_upload() {
    let conn = match get_connected_repo() {
        Some(c) => c,
        None => { ui::warning("未连接到远程仓库"); return; }
    };
    let (_url, cache_path) = conn;
    let registry = PlatformRegistry::new();

    let platform_name = match pick_platform("从哪个平台上传?") {
        Some(p) => p,
        None => return,
    };
    let platform = registry.get(&platform_name).unwrap();

    // 扫描本地 skill
    let mut local_skills = Vec::new();
    if platform.skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&platform.skills_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    let meta = metadata::parse_skill_md(&p.join("SKILL.md")).unwrap_or_default();
                    local_skills.push(SkillInfo { metadata: meta, category: "local".into(), source_path: p });
                }
            }
        }
    }
    local_skills.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    if local_skills.is_empty() {
        ui::warning(&format!("{} 平台暂无 skill", platform.label));
        return;
    }

    let names: Vec<String> = local_skills.iter().map(|s| format!("{}  {}", s.metadata.name, s.metadata.description)).collect();
    let selected = MultiSelect::new()
        .with_prompt("选择要上传的 skill")
        .items(&names)
        .interact_opt()
        .unwrap_or(None);

    let selected = match selected {
        Some(s) if !s.is_empty() => s,
        _ => { println!("  已取消"); return; }
    };

    // 选分类
    let skills_dir = cache_path.join("skills");
    let mut cats: Vec<String> = if skills_dir.is_dir() {
        std::fs::read_dir(&skills_dir)
            .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('_')).map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    cats.sort();
    if cats.is_empty() { cats.push("uncategorized".into()); }
    cats.push("+ 新建分类".into());

    let cat_idx = Select::new().with_prompt("选择分类").items(&cats).default(0).interact_opt().unwrap_or(None);
    let cat = match cat_idx {
        None => return,
        Some(i) if cats[i] == "+ 新建分类" => {
            let c: String = Input::new().with_prompt("输入分类名").interact_text().unwrap_or_default();
            if c.trim().is_empty() { return; }
            c.trim().to_string()
        }
        Some(i) => cats[i].clone(),
    };

    let sm = SkillManager::new(Some(cache_path.join("commands")));
    let git = match get_git() {
        Ok(g) => g,
        Err(e) => { ui::error(&format!("Git 错误: {e}"), None); return; }
    };

    let sp = Spinner::new(&format!("正在上传 {} 个 skill ...", selected.len()));
    let mut uploaded = Vec::new();
    for &idx in &selected {
        let skill = &local_skills[idx];
        let dest = cache_path.join("skills").join(&cat).join(&skill.metadata.name);
        if let Err(e) = crate::skills::copy_skill(&skill.source_path, &dest) {
            ui::error(&format!("复制 {} 失败: {e}", skill.metadata.name), None);
            continue;
        }
        uploaded.push(skill.metadata.name.clone());
    }

    let msg = if uploaded.len() == 1 {
        GitManager::build_commit_message("新增", &uploaded[0], &platform_name, &cat, "", "")
    } else {
        format!("📦 批量上传 {} 个 skill: {}\n\n来源: {} | 分类: {cat}", uploaded.len(), uploaded.join(", "), platform_name)
    };

    let _ = git.add_commit_push(&cache_path, &msg, true);
    let _ = sm.sync_all(&cache_path);
    sp.finish();

    for name in &uploaded {
        ui::success(name);
    }
}

fn action_search() {
    let keyword: String = Input::new().with_prompt("搜索关键词").interact_text().unwrap_or_default();
    if keyword.trim().is_empty() { return; }
    let keyword = keyword.trim().to_string();

    let conn = get_connected_repo();
    if let Some((_url, cache_path)) = conn {
        let sm = SkillManager::new(Some(cache_path.join("commands")));
        let available = sm.discover_skills(&cache_path.join("skills"));
        let matched = sm.search_skills(&available, &keyword);
        if matched.is_empty() {
            ui::info(&format!("未找到匹配 '{keyword}' 的 skill"));
        } else {
            let owned: Vec<_> = matched.into_iter().cloned().collect();
            ui::print_skill_table(&owned, &format!("搜索结果: '{keyword}' ({} 个)", owned.len()));
        }
    } else {
        ui::warning("未连接到远程仓库");
    }
}

fn action_update() {
    let conn = match get_connected_repo() {
        Some(c) => c,
        None => { ui::warning("未连接到远程仓库"); return; }
    };
    let (_url, cache_path) = conn;

    let platform = match pick_platform("更新哪个平台的 skill?") {
        Some(p) => p,
        None => return,
    };

    let git = match get_git() {
        Ok(g) => g,
        Err(e) => { ui::error(&format!("Git 错误: {e}"), None); return; }
    };

    let sp = Spinner::new("正在拉取远程仓库最新内容 ...");
    let _ = git.pull(&cache_path);
    sp.finish();

    let sm = SkillManager::new(Some(cache_path.join("commands")));
    let (new, updated, unchanged) = match sm.diff_skills(&cache_path.join("skills"), &platform) {
        Ok(r) => r,
        Err(e) => { ui::error(&format!("对比失败: {e}"), None); return; }
    };

    if new.is_empty() && updated.is_empty() {
        ui::success("所有已安装的 skill 均为最新。");
        return;
    }

    ui::print_update_table(&new, &updated, &unchanged);

    if updated.is_empty() {
        if !new.is_empty() {
            ui::info(&format!("有 {} 个新 skill 可用，使用「安装 Skill」菜单安装。", new.len()));
        }
        return;
    }

    let ok = Confirm::new().with_prompt(format!("更新 {} 个 skill?", updated.len())).default(true).interact_opt().unwrap_or(None);
    if ok != Some(true) { return; }

    let sp = Spinner::new(&format!("正在更新 {} 个 skill ...", updated.len()));
    for s in &updated {
        let _ = sm.install_skill(s, &platform);
    }
    sp.finish();
    ui::success(&format!("已更新 {} 个 skill。", updated.len()));
}

fn action_remove() {
    let platform = match pick_platform("从哪个平台卸载?") {
        Some(p) => p,
        None => return,
    };

    let sm = SkillManager::new(None);
    let installed = match sm.list_installed(&platform) {
        Ok(i) => i,
        Err(e) => { ui::error(&format!("读取失败: {e}"), None); return; }
    };

    if installed.is_empty() {
        ui::warning("暂无已安装的 skill");
        return;
    }

    let names: Vec<String> = installed.iter().map(|s| format!("{}  {}", s.metadata.name, s.metadata.description)).collect();
    let selected = MultiSelect::new()
        .with_prompt("选择要卸载的 skill")
        .items(&names)
        .interact_opt()
        .unwrap_or(None);

    let selected = match selected {
        Some(s) if !s.is_empty() => s,
        _ => { println!("  已取消"); return; }
    };

    let ok = Confirm::new().with_prompt(format!("确认卸载 {} 个 skill?", selected.len())).default(false).interact_opt().unwrap_or(None);
    if ok != Some(true) { return; }

    for &idx in &selected {
        let name = &installed[idx].metadata.name;
        match sm.remove_skill(name, &platform) {
            Ok(true) => ui::success(&format!("已卸载 {name}")),
            _ => ui::error(&format!("卸载 {name} 失败"), None),
        }
    }
}

fn action_version_mgmt() {
    let choices = vec!["📜  查看变更历史", "📌  安装指定版本", "← 返回"];
    let idx = Select::new().with_prompt("版本管理").items(&choices).default(0).interact_opt().unwrap_or(None);
    match idx {
        Some(0) => sub_history(),
        Some(1) => sub_pin_install(),
        _ => {}
    }
}

fn sub_history() {
    let conn = match get_connected_repo() {
        Some(c) => c,
        None => { ui::warning("未连接到远程仓库"); return; }
    };
    let (_url, cache_path) = conn;
    let sm = SkillManager::new(Some(cache_path.join("commands")));
    let available = sm.discover_skills(&cache_path.join("skills"));

    if available.is_empty() {
        ui::warning("仓库中暂无 skill");
        return;
    }

    let names: Vec<&str> = available.iter().map(|s| s.metadata.name.as_str()).collect();
    let idx = Select::new().with_prompt("查看哪个 skill 的历史?").items(&names).interact_opt().unwrap_or(None);
    let idx = match idx { Some(i) => i, None => return };
    let skill_name = names[idx];

    let skill_path = match GitManager::find_skill_path(&cache_path, skill_name) {
        Some(p) => p,
        None => { ui::error(&format!("未找到 '{skill_name}' 的路径"), None); return; }
    };

    let commits = GitManager::skill_log(&cache_path, &skill_path, 20).unwrap_or_default();
    if commits.is_empty() {
        println!("  '{skill_name}' 暂无变更历史");
        return;
    }

    ui::print_history_table(&commits, &format!("'{skill_name}' 变更历史"));
}

fn sub_pin_install() {
    let conn = match get_connected_repo() {
        Some(c) => c,
        None => { ui::warning("未连接到远程仓库"); return; }
    };
    let (_url, cache_path) = conn;
    let sm = SkillManager::new(Some(cache_path.join("commands")));
    let available = sm.discover_skills(&cache_path.join("skills"));

    if available.is_empty() {
        ui::warning("仓库中暂无 skill");
        return;
    }

    let names: Vec<&str> = available.iter().map(|s| s.metadata.name.as_str()).collect();
    let idx = Select::new().with_prompt("选择 skill").items(&names).interact_opt().unwrap_or(None);
    let idx = match idx { Some(i) => i, None => return };
    let skill_name = names[idx];

    let platform = match pick_platform("安装到哪个平台?") {
        Some(p) => p,
        None => return,
    };

    // 安装最新版本
    let matched: Vec<_> = available.iter().filter(|s| s.metadata.name == skill_name).collect();
    if let Some(skill) = matched.first() {
        let sp = Spinner::new(&format!("正在安装 {skill_name} ..."));
        let _ = sm.install_skill(skill, &platform);
        sp.finish();
        ui::success(&format!("已安装 '{skill_name}' 到 {platform}"));
    }
}

fn action_repo() {
    let config = get_config();
    let all_repos = config.get_repos().unwrap_or_default();

    if !all_repos.is_empty() {
        let current_url = config.get("repo.url").ok().flatten().unwrap_or_default();
        let current_alias = all_repos.iter().find(|(_, r)| r.url == current_url).map(|(a, _)| a.as_str());
        ui::print_repos_table(&all_repos, current_alias);
    }

    let choices = vec!["连接已有仓库", "初始化新仓库", "断开连接", "← 返回"];
    let idx = Select::new().with_prompt("操作").items(&choices).default(0).interact_opt().unwrap_or(None);

    match idx {
        Some(0) | Some(1) => {
            let is_init = idx == Some(1);
            let alias: String = Input::new().with_prompt("仓库别名").default("default".into()).interact_text().unwrap_or_default();
            let url: String = Input::new().with_prompt("Git 仓库 URL").interact_text().unwrap_or_default();
            if url.trim().is_empty() { return; }
            let url = url.trim().to_string();

            if !GitManager::validate_url(&url) {
                ui::error("无效 URL", Some("支持 https://... 或 git@...:..."));
                return;
            }

            let git = match get_git() {
                Ok(g) => g,
                Err(e) => { ui::error(&format!("Git 错误: {e}"), None); return; }
            };

            let sp = Spinner::new(&format!("正在克隆 {url} ..."));
            let repo_path = match git.clone(&url) {
                Ok(p) => p,
                Err(e) => { sp.finish(); ui::error(&e.to_string(), None); return; }
            };
            sp.finish();

            if is_init && !GitManager::has_skills_dir(&repo_path) {
                let sp = Spinner::new("正在创建仓库结构 ...");
                let _ = GitManager::init_repo_structure(&repo_path);
                sp.finish();
                let _ = git.add_commit_push(&repo_path, "初始化 skill 仓库结构", true);
                ui::success("已初始化并推送");
            }

            let _ = config.add_repo(&alias, &url, &repo_path.to_string_lossy());
            ui::success(&format!("已连接 (别名: {alias})"));
        }
        Some(2) => {
            if all_repos.is_empty() {
                println!("  当前未连接");
                return;
            }
            let aliases: Vec<&str> = all_repos.keys().map(|s| s.as_str()).collect();
            let idx = Select::new().with_prompt("选择要断开的仓库").items(&aliases).interact_opt().unwrap_or(None);
            if let Some(i) = idx {
                let _ = config.remove_repo(aliases[i]);
                ui::success(&format!("已断开 '{}'", aliases[i]));
            }
        }
        _ => {}
    }
}

fn action_settings() {
    let config = get_config();

    loop {
        let default_platform = config.get("defaults.target_platform").ok().flatten().unwrap_or_else(|| "未设置".into());
        let branch_mode = config.get("branch.mode").ok().flatten().unwrap_or_else(|| "direct".into());
        let auto_merge = config.get("branch.auto_merge").ok().flatten().unwrap_or_else(|| "true".into());
        let cleanup = config.get("branch.cleanup").ok().flatten().unwrap_or_else(|| "true".into());

        println!();
        println!("  {}", style("当前配置").bold().cyan());
        println!("  {:<24} {:<16} {}", style("配置项").bold(), style("当前值").cyan(), style("说明").dim());
        println!("  {}", style("─".repeat(64)).dim());
        println!("  {:<24} {:<16} {}", "默认平台", default_platform, style("安装/更新时的默认平台").dim());
        println!("  {:<24} {:<16} {}", "分支模式", branch_mode, style("direct=直推 / branch=分支协作").dim());
        println!("  {:<24} {:<16} {}", "自动合并", if auto_merge == "false" { "关闭" } else { "开启" }, style("无冲突时自动合并").dim());
        println!("  {:<24} {:<16} {}", "自动清理分支", if cleanup == "false" { "关闭" } else { "开启" }, style("合并后删除远程分支").dim());
        println!();

        let choices = vec!["修改默认平台", "修改分支模式", "修改自动合并", "修改自动清理分支", "← 返回"];
        let idx = Select::new().with_prompt("修改配置").items(&choices).default(0).interact_opt().unwrap_or(None);

        match idx {
            None | Some(4) => return,
            Some(0) => {
                let registry = PlatformRegistry::new();
                let mut names: Vec<String> = registry.all().iter().map(|p| p.name.clone()).collect();
                names.push("清除默认".into());
                let idx = Select::new().with_prompt("选择默认平台").items(&names).interact_opt().unwrap_or(None);
                if let Some(i) = idx {
                    if names[i] == "清除默认" {
                        let _ = config.delete("defaults.target_platform");
                        ui::success("已清除默认平台。");
                    } else {
                        let _ = config.set("defaults.target_platform", &names[i]);
                        ui::success(&format!("默认平台已设为 {}。", names[i]));
                    }
                }
            }
            Some(1) => {
                let modes = vec!["direct — 直接推送到主分支", "branch — 创建个人分支再合并"];
                let idx = Select::new().with_prompt("选择分支模式").items(&modes).interact_opt().unwrap_or(None);
                if let Some(i) = idx {
                    let val = if i == 0 { "direct" } else { "branch" };
                    let _ = config.set("branch.mode", val);
                    ui::success(&format!("已切换到 {}。", if i == 0 { "直推模式" } else { "分支协作模式" }));
                }
            }
            Some(2) => {
                let opts = vec!["开启", "关闭"];
                let idx = Select::new().with_prompt("自动合并").items(&opts).interact_opt().unwrap_or(None);
                if let Some(i) = idx {
                    let val = if i == 0 { "true" } else { "false" };
                    let _ = config.set("branch.auto_merge", val);
                    ui::success(&format!("自动合并已{}。", opts[i]));
                }
            }
            Some(3) => {
                let opts = vec!["开启", "关闭"];
                let idx = Select::new().with_prompt("自动清理分支").items(&opts).interact_opt().unwrap_or(None);
                if let Some(i) = idx {
                    let val = if i == 0 { "true" } else { "false" };
                    let _ = config.set("branch.cleanup", val);
                    ui::success(&format!("自动清理分支已{}。", opts[i]));
                }
            }
            _ => {}
        }
    }
}
