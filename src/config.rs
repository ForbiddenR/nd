use std::{fs, io::ErrorKind, time::Duration};

use anyhow::{Context, Result};
use reqwest::IntoUrl;
use serde::Deserialize;
use serde_json::Value;

const CONFIG_PATH: &str = "nd.toml";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BuildConfig {
    #[serde(default)]
    pub configs: Vec<Config>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    pub tag_template: Option<String>,
    pub tag: Option<String>,
    pub latest_version: Option<String>,
    pub version_url: Option<String>,
}

pub fn read_build_config() -> Result<BuildConfig> {
    let contents = match fs::read_to_string(CONFIG_PATH) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BuildConfig::default()),
        Err(err) => return Err(err).context("failed to read nd.toml"),
    };

    resolve_build_config(toml::from_str::<BuildConfig>(&contents)?)
}

fn resolve_build_config(mut config: BuildConfig) -> Result<BuildConfig> {
    for item in &mut config.configs {
        if let Some(url) = item.version_url.as_deref() {
            let latest_version = fetch_npm_latest_version(url)?;
            item.latest_version = Some(latest_version.clone());

            if let Some(tag_template) = item.tag_template.as_deref() {
                item.tag = Some(tag_template.replace("{version}", &latest_version));
            }
        } else if item.tag.is_none() {
            item.tag = item.tag_template.clone();
        }
    }

    Ok(config)
}

fn fetch_npm_latest_version<T: IntoUrl>(url: T) -> Result<String> {
    let resp = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(3))
        .build()?
        .get(url)
        .send()?
        .error_for_status()?;

    let body = resp.json::<Value>()?;
    let version = body
        .get("version")
        .or_else(|| body.get("dist-tags").and_then(|tags| tags.get("latest")))
        .and_then(|version| version.as_str())
        .context("missing npm latest version")?;

    Ok(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::{BuildConfig, Config, resolve_build_config};

    #[test]
    fn resolves_static_tag_from_template_without_url() {
        let config = BuildConfig {
            configs: vec![Config {
                tag_template: Some("example/app:latest".to_string()),
                ..Config::default()
            }],
        };

        let resolved = resolve_build_config(config).unwrap();

        assert_eq!(
            resolved.configs[0].tag,
            Some("example/app:latest".to_string())
        );
        assert_eq!(resolved.configs[0].latest_version, None);
    }

    #[test]
    fn parses_config_list() {
        let contents = r#"
[[configs]]
tag_template = "example/app:{version}"
version_url = "https://registry.npmjs.org/example/latest"

[[configs]]
tag = "example/other:latest"
"#;

        let config = toml::from_str::<BuildConfig>(contents).unwrap();

        assert_eq!(config.configs.len(), 2);
        assert_eq!(
            config.configs[0].tag_template,
            Some("example/app:{version}".to_string())
        );
        assert_eq!(
            config.configs[0].version_url,
            Some("https://registry.npmjs.org/example/latest".to_string())
        );
        assert_eq!(
            config.configs[1].tag,
            Some("example/other:latest".to_string())
        );
    }
}
