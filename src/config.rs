use anyhow::{Context, Result};
use indexmap::map::IndexMap;
use serde::de::{self, Deserialize, Deserializer, Error};
use serde_derive::Deserialize;
use std::fs::{create_dir, File};
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

const DEFAULT_FALLBACK_ICON: &str = "-";
const DEFAULT_SEPARATOR: &str = ": ";
const DEFAULT_CONFIG: &str = include_str!("../default_config.toml");

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub mappings: IndexMap<String, String>,
    pub other: Other,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Other {
    pub fallback_icon: Option<String>,
    pub separator: Option<String>,
    pub deduplicate_icons: bool,
}

impl Config {
    pub fn new() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let mut buf = String::new();
            File::open(path)
                .and_then(|f| BufReader::new(f).read_to_string(&mut buf))
                .context("Failed to read configuration file")?;
            Ok(toml::from_str(&buf)?)
        } else {
            File::create(path)
                .and_then(|mut f| f.write_all(DEFAULT_CONFIG.as_bytes()))
                .context("Failed to create default configuration file")?;
            Ok(toml::from_str(DEFAULT_CONFIG)?)
        }
    }

    #[cfg(test)]
    pub(crate) fn from_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("Failed to parse config as toml")
    }

    pub fn fallback_icon(&self) -> &str {
        self.other
            .fallback_icon
            .as_deref()
            .unwrap_or(DEFAULT_FALLBACK_ICON)
    }

    pub fn separator(&self) -> &str {
        let sep = self.other.separator.as_deref();
        if let Some(sep) = sep {
            let fallback_icon = self.fallback_icon();
            if let Some(icon) = self.mappings.values().find(|icon| icon.contains(sep)) {
                error!("Can't use separator: \"{sep}\" as it is contained in icon: \"{icon}\".");
                DEFAULT_SEPARATOR
            } else if fallback_icon.contains(sep) {
                error!("Can't use separator: \"{sep}\" as it is contained in fallback icon: \"{fallback_icon}\"");
                DEFAULT_SEPARATOR
            } else {
                sep
            }
        } else {
            DEFAULT_SEPARATOR
        }
    }

    pub fn path() -> Result<PathBuf> {
        let mut user_path = dirs::config_dir().context("Could not find the configuration path")?;
        let mut system_path = PathBuf::from("/etc/xdg");

        for path in [&mut user_path, &mut system_path] {
            path.push(env!("CARGO_PKG_NAME"));
            path.push("config.toml");
        }
        let path = if system_path.exists() && !user_path.exists() {
            system_path
        } else {
            user_path
        };
        let dir = path
            .parent()
            .context("Expected path to contain a parent directory")?;
        if !dir.exists() {
            create_dir(dir).context("Failed to create configuration directory")?;
        }
        Ok(path)
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Config;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("workstyle configuration map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut config = Config::default();
                while let Some((key, value)) = map.next_entry::<String, toml::Value>()? {
                    if key == "other" {
                        config.other = Other::deserialize(value).map_err(A::Error::custom)?;
                    } else {
                        config
                            .mappings
                            .insert(key, String::deserialize(value).map_err(A::Error::custom)?);
                    }
                }
                Ok(config)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
