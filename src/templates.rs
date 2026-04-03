//! 仓库初始化模板（内嵌字符串，编译时包含）

pub const ROOT_README: &str = r#"# Skill Repo

团队 Skill 共享仓库。使用 `skill-repo` CLI 工具管理。
"#;

pub const SKILLS_README: &str = r#"# Skills

<!-- BEGIN AUTO SKILLS -->
<!-- END AUTO SKILLS -->
"#;

pub const GITIGNORE: &str = r#"__pycache__/
*.pyc
.venv/
*.egg-info/
dist/
build/
"#;

pub const PYPROJECT_TOML: &str = r#"[project]
name = "skill-repo-content"
version = "0.1.0"
description = "Skill 仓库内容"
"#;

pub const PREK_TOML: &str = r#"[hooks.post-commit]
run = "python scripts/post_commit_sync.py"
"#;

pub const MANIFEST_JSON: &str = r#"{
  "name": "skill-repo",
  "version": "1.0.0",
  "skills": []
}
"#;

pub const POST_COMMIT_SYNC: &str = r#"#!/usr/bin/env python3
"""Post-commit hook: sync README, commands, manifest."""
import subprocess, sys
scripts = ["sync_skills_readme.py", "sync_commands.py", "sync_claude_marketplace.py"]
for s in scripts:
    subprocess.run([sys.executable, f"scripts/{s}"], check=False)
"#;

pub const SYNC_COMMANDS: &str = r#"#!/usr/bin/env python3
"""Sync commands/*.md from skills."""
print("sync_commands: placeholder")
"#;

pub const SYNC_SKILLS_README: &str = r#"#!/usr/bin/env python3
"""Sync skills/README.md table."""
print("sync_skills_readme: placeholder")
"#;

pub const SYNC_CLAUDE_MARKETPLACE: &str = r#"#!/usr/bin/env python3
"""Sync .claude-plugin/manifest.json."""
print("sync_claude_marketplace: placeholder")
"#;

pub const DEFAULT_SKILL_MD: &str = r#"---
name: "skill-repo-cli"
description: "Skill Repo CLI 操作指南"
version: "0.1.0"
author: "skill-repo"
---

# skill-repo-cli

Skill Repo CLI 工具的使用指南。
"#;
