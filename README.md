<div align="center">

# Skill Repo (Rust)

单二进制，零依赖，跨平台开箱即用的 Code Agent Skill 管理工具

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](../LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)]()

</div>

---

## 安装

### 方式一：从 GitHub Releases 下载（推荐，无需 Rust 环境）

前往 [Releases](https://github.com/weijunjiang123/skill-repo-rs/releases) 页面下载：

| 平台 | 文件名 |
|:-----|:-------|
| Windows x64 | `skill-repo-x86_64-pc-windows-msvc.zip` |
| macOS Apple Silicon | `skill-repo-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `skill-repo-x86_64-apple-darwin.tar.gz` |
| Linux x64 | `skill-repo-x86_64-unknown-linux-gnu.tar.gz` |

下载解压后放到 PATH 中：

**Windows (PowerShell):**

```powershell
Move-Item skill-repo.exe $env:USERPROFILE\.cargo\bin\
```

**macOS / Linux:**

```bash
tar xzf skill-repo-*.tar.gz
sudo mv skill-repo /usr/local/bin/
```

### 方式二：cargo install（需要 Rust 环境）

```bash
cargo install --git https://github.com/weijunjiang123/skill-repo.git --path skill-repo-rs

skill-repo --help
```

### 方式三：一键安装脚本

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/weijunjiang123/skill-repo/master/skill-repo-rs/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/weijunjiang123/skill-repo/master/skill-repo-rs/install.ps1 | iex
```

### 方式四：从源码构建

```bash
git clone https://github.com/weijunjiang123/skill-repo.git
cd skill-repo/skill-repo-rs
cargo build --release
# 二进制在 target/release/skill-repo(.exe)
```

## 快速上手

```bash
# 1. 连接团队的 Skill 仓库
skill-repo connect git@github.com:your-team/skills.git

# 2. 看看有什么 Skill
skill-repo install --target kiro --list

# 3. 装一个试试
skill-repo install --target kiro --skill code-review

# 4. 把自己的 Skill 分享出去
skill-repo upload --source kiro --skill my-skill --category tools

# 5. 不想记命令？交互式菜单
skill-repo interactive
```

## 命令参考

```
skill-repo connect <git-url> [--alias name]     连接远程仓库
skill-repo init <git-url> [--alias name]        初始化空仓库
skill-repo status                               查看仓库状态

skill-repo install --target <platform> [--skill name | --all | --list]
skill-repo upload --source <platform> --skill <name> [--category cat]
skill-repo search <keyword>
skill-repo update --target <platform> [--dry-run]
skill-repo remove --target <platform> --skill <name> [-y]
skill-repo diff --skill <name> --target <platform>
skill-repo create --name <name> [--description desc] [--target platform]

skill-repo history --skill <name> [--limit 20]
skill-repo rollback --skill <name> --to <commit> [--push]
skill-repo pin --skill <name> [--commit hash] --target <platform>

skill-repo config show
skill-repo config set <key> <value>
skill-repo branch list | merge <branch> | mode <direct|branch>

skill-repo interactive                          交互式 TUI
```

支持的平台: `claude` / `codex` / `kiro`

## 与 Python 版的区别

- 单二进制文件，无需 Python 环境
- 启动速度更快（~5ms vs ~500ms）
- 发布体积更小（~2.5MB vs ~30MB+）
- 功能完全对等
