//! A preview stages the exact package to be installed. Confirmation consumes
//! those files rather than fetching a potentially different revision.

use super::*;
use crate::plugin::{presentation, PluginContents};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginPreview {
    pub token: String,
    pub name: String,
    pub version: Option<String>,
    pub contents: PluginContents,
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    id: String,
    content: String,
    revision: Option<String>,
    version: Option<String>,
}

impl Installer {
    fn review_dir(&self, token: &str) -> Result<PathBuf, CoreError> {
        let uuid = uuid::Uuid::parse_str(token).map_err(|_| err("invalid plugin review token"))?;
        Ok(self
            .config_dir
            .join(CACHE_DIR)
            .join(format!(".review-{}", uuid.simple())))
    }

    pub fn preview(&self, id: &str) -> Result<PluginPreview, CoreError> {
        let (name, market) = id
            .split_once('@')
            .ok_or_else(|| err("expected plugin@marketplace"))?;
        let known = self.find(market)?;
        let manifest = self.manifest(&known)?;
        let entry = manifest
            .plugins
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| err("plugin is no longer in the catalog"))?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let staging = self.review_dir(&token)?;
        std::fs::create_dir_all(&staging).map_err(|e| err(e.to_string()))?;
        let result = (|| {
            let fetched =
                fetch::from_entry(&known.dir(&self.config_dir), &manifest, entry, &staging)
                    .map_err(err)?;
            let root = std::fs::canonicalize(&fetched.package).map_err(|e| err(e.to_string()))?;
            let (_, c) = read_package(&root, Some(name)).map_err(err)?;
            write_json(
                &staging.join("review.json"),
                &Receipt {
                    id: id.into(),
                    content: registry::content_hash(&root),
                    revision: fetched.revision,
                    version: entry.version.clone(),
                },
            )?;
            Ok(PluginPreview {
                token,
                name: c.name,
                version: c.version,
                contents: presentation::contents(
                    &root,
                    &c.skill_dirs,
                    &c.commands,
                    &c.mcp_servers,
                    &c.hooks,
                ),
                notes: c.notes,
            })
        })();
        if result.is_err() {
            git::remove_tree(&staging);
        }
        result
    }

    pub fn discard_preview(&self, token: &str) -> Result<(), CoreError> {
        git::remove_tree(&self.review_dir(token)?);
        Ok(())
    }

    pub fn install_preview(&self, token: &str) -> Result<PluginSummary, CoreError> {
        let staging = self.review_dir(token)?;
        let result = (|| {
            let receipt: Receipt = serde_json::from_str(
                &std::fs::read_to_string(staging.join("review.json"))
                    .map_err(|e| err(e.to_string()))?,
            )
            .map_err(|e| err(e.to_string()))?;
            let package = staging.join("package");
            if registry::content_hash(&package) != receipt.content {
                return Err(err(
                    "reviewed plugin files changed; review the plugin again",
                ));
            }
            let (name, market) = receipt
                .id
                .split_once('@')
                .ok_or_else(|| err("invalid reviewed plugin id"))?;
            self.put(
                &receipt.id,
                name,
                market,
                receipt.version,
                From::Marketplace {
                    marketplace: market.into(),
                    entry: name.into(),
                },
                |_| {
                    Ok(fetch::Fetched {
                        package,
                        revision: receipt.revision,
                    })
                },
            )
            .map(|(s, _)| s)
        })();
        git::remove_tree(&staging);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn local_market(market: &TempDir) {
        market.write(".claude-plugin/marketplace.json", r#"{"name":"review-market","owner":{"name":"test"},"plugins":[{"name":"p","source":"./p"}]}"#);
        market.write(
            "p/.claude-plugin/plugin.json",
            r#"{"name":"p","version":"1.0.0"}"#,
        );
        market.write("p/hooks/hooks.json", r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo original"}]}]}}"#);
    }

    #[test]
    fn confirming_installs_the_reviewed_files_even_if_the_source_changes() {
        let config = TempDir::new();
        let market = TempDir::new();
        local_market(&market);
        let i = Installer::new(config.path(), None);
        i.add_marketplace(&plain(market.path())).unwrap();
        let preview = i.preview("p@review-market").unwrap();
        assert!(PluginSources::new(config.path(), None)
            .load()
            .plugins()
            .is_empty());
        assert_eq!(preview.contents.hooks[0].command, "echo original");
        market.write("p/hooks/hooks.json", r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo changed"}]}]}}"#);
        let installed = i.install_preview(&preview.token).unwrap();
        assert_eq!(installed.contents.hooks[0].command, "echo original");
        assert!(!i.review_dir(&preview.token).unwrap().exists());
    }

    #[test]
    fn changed_staging_is_rejected_and_cancellation_never_installs() {
        let config = TempDir::new();
        let market = TempDir::new();
        local_market(&market);
        let i = Installer::new(config.path(), None);
        i.add_marketplace(&plain(market.path())).unwrap();
        let preview = i.preview("p@review-market").unwrap();
        std::fs::write(
            i.review_dir(&preview.token)
                .unwrap()
                .join("package/extra.txt"),
            "changed",
        )
        .unwrap();
        assert!(i.install_preview(&preview.token).is_err());
        let preview = i.preview("p@review-market").unwrap();
        i.discard_preview(&preview.token).unwrap();
        assert!(!i.review_dir(&preview.token).unwrap().exists());
        assert!(PluginSources::new(config.path(), None)
            .load()
            .plugins()
            .is_empty());
        assert!(i.discard_preview("../outside").is_err());
    }
}
