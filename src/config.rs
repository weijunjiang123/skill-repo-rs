//! 配置管理器 — 读写 TOML 格式的配置文件

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// TOML 配置管理器
pub struct ConfigManager {
    pub config_path: PathBuf,
}

impl ConfigManager {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self {
            config_path: config_path.unwrap_or_else(Self::default_path),
        }
    }

    /// 加载配置，不存在则返回空表
    pub fn load(&self) -> Result<toml::Table> {
        if !self.config_path.exists() {
            return Ok(toml::Table::new());
        }
        let content = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("无法读取配置 {}", self.config_path.display()))?;
        let table: toml::Table = content.parse().with_context(|| "配置文件格式错误")?;
        Ok(table)
    }

    /// 保存配置
    pub fn save(&self, config: &toml::Table) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// 获取配置项，支持点号分隔（如 "repo.url"）
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let config = self.load()?;
        let parts: Vec<&str> = key.split('.').collect();
        let mut current: &toml::Value = &toml::Value::Table(config);

        for part in &parts {
            match current {
                toml::Value::Table(t) => match t.get(*part) {
                    Some(v) => current = v,
                    None => return Ok(None),
                },
                _ => return Ok(None),
            }
        }

        match current {
            toml::Value::String(s) => Ok(Some(s.clone())),
            other => Ok(Some(other.to_string())),
        }
    }

    /// 设置配置项，支持点号分隔，自动创建中间表
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut config = self.load()?;
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &mut config;

        for part in &parts[..parts.len() - 1] {
            if !current.contains_key(*part)
                || !current[*part].is_table()
            {
                current.insert(
                    part.to_string(),
                    toml::Value::Table(toml::Table::new()),
                );
            }
            current = current
                .get_mut(*part)
                .unwrap()
                .as_table_mut()
                .unwrap();
        }

        current.insert(
            parts.last().unwrap().to_string(),
            toml::Value::String(value.to_string()),
        );
        self.save(&config)
    }

    /// 删除配置项
    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut config = self.load()?;
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &mut config;

        for part in &parts[..parts.len() - 1] {
            match current.get_mut(*part) {
                Some(toml::Value::Table(t)) => current = t,
                _ => return Ok(false),
            }
        }

        let removed = current.remove(*parts.last().unwrap()).is_some();
        if removed {
            self.save(&config)?;
        }
        Ok(removed)
    }

    /// 获取所有已连接的仓库 {alias: {url, cache_path}}
    pub fn get_repos(&self) -> Result<BTreeMap<String, RepoInfo>> {
        let config = self.load()?;

        // 优先读 [repos]
        if let Some(toml::Value::Table(repos)) = config.get("repos") {
            let mut result = BTreeMap::new();
            for (alias, val) in repos {
                if let toml::Value::Table(t) = val {
                    result.insert(
                        alias.clone(),
                        RepoInfo {
                            url: t
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            cache_path: t
                                .get("cache_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        },
                    );
                }
            }
            if !result.is_empty() {
                return Ok(result);
            }
        }

        // 向后兼容旧 [repo]
        if let Some(toml::Value::Table(repo)) = config.get("repo") {
            if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                if !url.is_empty() {
                    let mut result = BTreeMap::new();
                    result.insert(
                        "default".to_string(),
                        RepoInfo {
                            url: url.to_string(),
                            cache_path: repo
                                .get("cache_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        },
                    );
                    return Ok(result);
                }
            }
        }

        Ok(BTreeMap::new())
    }

    /// 添加或更新仓库连接
    pub fn add_repo(&self, alias: &str, url: &str, cache_path: &str) -> Result<()> {
        let mut config = self.load()?;

        // 更新 [repos]
        if !config.contains_key("repos") || !config["repos"].is_table() {
            config.insert("repos".into(), toml::Value::Table(toml::Table::new()));
        }
        let repos = config["repos"].as_table_mut().unwrap();
        let mut entry = toml::Table::new();
        entry.insert("url".into(), toml::Value::String(url.into()));
        entry.insert("cache_path".into(), toml::Value::String(cache_path.into()));
        repos.insert(alias.into(), toml::Value::Table(entry));

        // 向后兼容 [repo]
        if !config.contains_key("repo") {
            config.insert("repo".into(), toml::Value::Table(toml::Table::new()));
        }
        let repo = config["repo"].as_table_mut().unwrap();
        repo.insert("url".into(), toml::Value::String(url.into()));
        repo.insert("cache_path".into(), toml::Value::String(cache_path.into()));

        self.save(&config)
    }

    /// 移除仓库连接
    pub fn remove_repo(&self, alias: &str) -> Result<bool> {
        let mut config = self.load()?;

        // 先从 repos 中移除并收集需要的信息
        let (removed, is_empty, first_url, first_cache) = {
            let repos = match config.get_mut("repos") {
                Some(toml::Value::Table(t)) => t,
                _ => return Ok(false),
            };

            if repos.remove(alias).is_none() {
                return Ok(false);
            }

            let is_empty = repos.is_empty();
            let (first_url, first_cache) = if !is_empty {
                let first = repos.values().next().unwrap();
                if let toml::Value::Table(t) = first {
                    (
                        t.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        t.get("cache_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    )
                } else {
                    (String::new(), String::new())
                }
            } else {
                (String::new(), String::new())
            };

            (true, is_empty, first_url, first_cache)
        };

        if !removed {
            return Ok(false);
        }

        // 更新 [repo] 兼容字段
        if let Some(toml::Value::Table(repo)) = config.get_mut("repo") {
            if is_empty {
                repo.insert("url".into(), toml::Value::String(String::new()));
                repo.insert("cache_path".into(), toml::Value::String(String::new()));
            } else {
                repo.insert("url".into(), toml::Value::String(first_url));
                repo.insert("cache_path".into(), toml::Value::String(first_cache));
            }
        }

        self.save(&config)?;
        Ok(true)
    }

    /// 获取指定别名的仓库信息
    pub fn get_repo(&self, alias: &str) -> Result<Option<RepoInfo>> {
        let repos = self.get_repos()?;
        Ok(repos.get(alias).cloned())
    }

    /// 默认配置路径
    fn default_path() -> PathBuf {
        if cfg!(windows) {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("skill-repo")
                .join("config.toml")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("skill-repo")
                .join("config.toml")
        }
    }
}

/// 仓库信息
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub url: String,
    pub cache_path: String,
}
