//! 终端输出工具 — 统一输出风格

use console::style;

/// UTF-8 安全截断：按字符数截断，不会切到多字节字符中间
fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub fn success(msg: &str) {
    println!("  {} {msg}", style("✓").green());
}

pub fn error(msg: &str, hint: Option<&str>) {
    eprintln!("  {} {msg}", style("✗ 错误:").red().bold());
    if let Some(h) = hint {
        eprintln!("  {} {h}", style("💡").dim());
    }
}

pub fn warning(msg: &str) {
    eprintln!("  {} {msg}", style("⚠").yellow());
}

pub fn info(msg: &str) {
    println!("  {} {msg}", style("ℹ").cyan());
}

/// 打印 skill 列表表格
pub fn print_skill_table(skills: &[crate::metadata::SkillInfo], title: &str) {
    println!();
    println!("  {}", style(title).bold().cyan());
    println!(
        "  {:<4} {:<24} {:<16} {}",
        style("#").dim(),
        style("名称").bold(),
        style("分类").cyan(),
        style("描述").dim()
    );
    println!("  {}", style("─".repeat(72)).dim());

    for (i, s) in skills.iter().enumerate() {
        let desc = truncate_str(&s.metadata.description, 34);
        let desc = if desc.is_empty() { "—".to_string() } else { desc };
        println!(
            "  {:<4} {:<24} {:<16} {}",
            style(i + 1).dim(),
            style(&s.metadata.name).bold(),
            style(&s.category).cyan(),
            style(desc).dim()
        );
    }
    println!();
}

/// 打印 commit 历史表格
pub fn print_history_table(commits: &[crate::git::CommitInfo], title: &str) {
    println!();
    println!("  {}", style(title).bold().cyan());
    println!(
        "  {:<4} {:<10} {:<12} {:<14} {}",
        style("#").dim(),
        style("提交").yellow(),
        style("日期").dim(),
        style("作者").cyan(),
        style("说明")
    );
    println!("  {}", style("─".repeat(72)).dim());

    for (i, c) in commits.iter().enumerate() {
        let msg = truncate_str(&c.message, 40);
        println!(
            "  {:<4} {:<10} {:<12} {:<14} {}",
            style(i + 1).dim(),
            style(&c.short_hash).yellow(),
            style(&c.date).dim(),
            style(&c.author).cyan(),
            msg
        );
    }
    println!();
}

/// 打印更新对比表格
pub fn print_update_table(
    new: &[crate::metadata::SkillInfo],
    updated: &[crate::metadata::SkillInfo],
    unchanged: &[crate::metadata::SkillInfo],
) {
    println!();
    println!("  {}", style("更新检查").bold().cyan());
    println!(
        "  {:<24} {:<16} {}",
        style("名称").bold(),
        style("状态"),
        style("分类").cyan()
    );
    println!("  {}", style("─".repeat(56)).dim());

    for s in new {
        println!(
            "  {:<24} {:<16} {}",
            style(&s.metadata.name).bold(),
            style("🆕 新增").cyan(),
            style(&s.category).cyan()
        );
    }
    for s in updated {
        println!(
            "  {:<24} {:<16} {}",
            style(&s.metadata.name).bold(),
            style("📦 有更新").yellow(),
            style(&s.category).cyan()
        );
    }
    for s in unchanged {
        println!(
            "  {:<24} {:<16} {}",
            style(&s.metadata.name).bold(),
            style("✓ 最新").green(),
            style(&s.category).cyan()
        );
    }
    println!();
}

/// 打印仓库列表
pub fn print_repos_table(
    repos: &std::collections::BTreeMap<String, crate::config::RepoInfo>,
    current_alias: Option<&str>,
) {
    println!();
    println!("  {}", style("已连接仓库").bold().cyan());
    println!(
        "  {:<12} {:<48} {}",
        style("别名").bold(),
        style("URL"),
        style("状态")
    );
    println!("  {}", style("─".repeat(68)).dim());

    for (alias, info) in repos {
        let marker = if current_alias == Some(alias.as_str()) {
            style("● 当前").green().to_string()
        } else {
            style("○").dim().to_string()
        };
        println!("  {:<12} {:<48} {}", style(alias).bold(), info.url, marker);
    }
    println!();
}

/// Spinner 包装
pub struct Spinner {
    pb: indicatif::ProgressBar,
}

impl Spinner {
    pub fn new(msg: &str) -> Self {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { pb }
    }

    pub fn finish(self) {
        self.pb.finish_and_clear();
    }
}
