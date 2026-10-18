//! Installing from real git repositories on disk: no network, but every fetch
//! goes through git as it would for GitHub.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use super::*;
use crate::plugin::{PluginOrigin, PluginSources};
use crate::testutil::TempDir;

/// Runs git in `dir`, panicking with what it said if it fails.
fn sh_git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repository with what is written in it so far committed. Fetching a commit
/// by sha is allowed, as GitHub allows it.
fn repo(dir: &TempDir) -> PathBuf {
    let path = dir.path().to_path_buf();
    if !path.join(".git").exists() {
        sh_git(&path, &["init", "--quiet"]);
        sh_git(&path, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    }
    sh_git(&path, &["add", "-A"]);
    sh_git(&path, &["commit", "--quiet", "--allow-empty", "-m", "c"]);
    path
}

fn head(dir: &Path) -> String {
    sh_git(dir, &["rev-parse", "HEAD"])
}

fn file_url(path: &Path) -> String {
    // Not the verbatim `\\?\` form a canonical Windows path has: git reads
    // that as a network share.
    let text = crate::workspace::roots::plain(path);
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

fn skill(dir: &TempDir, at: &str, name: &str, body: &str) {
    dir.write(&format!("{at}/skills/{name}/SKILL.md"), &format!("---\nname: {name}\ndescription: d\n---\n{body}"));
}

/// A marketplace named `market` offering `plugins`, committed.
fn market(dir: &TempDir, plugins: Value) -> PathBuf {
    dir.write(
        ".claude-plugin/marketplace.json",
        &json!({ "name": "market", "owner": { "name": "t" }, "plugins": plugins }).to_string(),
    );
    repo(dir)
}

fn installer(config: &TempDir) -> Installer {
    Installer::new(config.path(), None)
}

fn loaded(config: &TempDir) -> crate::plugin::PluginCatalog {
    PluginSources::new(config.path(), None).load()
}

/// No staging directory is ever left behind.
fn no_leftovers(config: &TempDir) {
    let cache = config.path().join(CACHE_DIR);
    if let Ok(read) = std::fs::read_dir(&cache) {
        for e in read.flatten() {
            assert!(!e.file_name().to_string_lossy().starts_with('.'), "left behind: {:?}", e.path());
        }
    }
}

#[test]
fn a_git_marketplace_is_cloned_under_the_name_it_gives_itself() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    let path = market(&mkt, json!([{ "name": "a", "source": "./a" }]));

    let summary = installer(&config).add_marketplace(&file_url(&path)).unwrap();
    assert_eq!(summary.name, "market");
    assert_eq!(summary.origin, MarketplaceOrigin::Own);
    assert!(summary.fetched);
    assert_eq!(summary.plugins, Some(1));
    assert!(config.path().join("marketplaces/market/.claude-plugin/marketplace.json").is_file());
    no_leftovers(&config);
}

#[test]
fn a_folder_marketplace_is_read_where_it_is() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    mkt.write(".claude-plugin/marketplace.json", r#"{ "name": "local", "plugins": [] }"#);
    let summary = installer(&config).add_marketplace(&mkt.path().display().to_string()).unwrap();
    assert_eq!(summary.name, "local");
    assert!(summary.fetched);
    assert!(!config.path().join("marketplaces/local").exists(), "nothing is copied");
}

#[test]
fn the_official_marketplace_is_listed_and_not_fetched_until_asked() {
    let config = TempDir::new();
    let list = installer(&config).marketplaces();
    let official = list.iter().find(|m| m.name == marketplace::OFFICIAL).expect("built in");
    assert_eq!(official.origin, MarketplaceOrigin::Builtin);
    assert!(!official.fetched);
    assert_eq!(official.source, "anthropics/claude-plugins-official");
}

#[test]
fn claude_codes_marketplaces_are_listed_without_touching_its_files() {
    let (config, home, mkt) = (TempDir::new(), TempDir::new(), TempDir::new());
    mkt.write(".claude-plugin/marketplace.json", r#"{ "name": "theirs", "plugins": [] }"#);
    home.write(
        "plugins/known_marketplaces.json",
        &json!({ "theirs": { "source": { "source": "directory", "path": mkt.path() } } }).to_string(),
    );
    let before = std::fs::read_to_string(home.path().join("plugins/known_marketplaces.json")).unwrap();

    let installer = Installer::new(config.path(), Some(home.path().to_path_buf()));
    let theirs = installer.marketplaces().into_iter().find(|m| m.name == "theirs").unwrap();
    assert_eq!(theirs.origin, MarketplaceOrigin::ClaudeCode);
    assert!(theirs.fetched);

    installer.remove_marketplace("theirs").unwrap();
    assert!(installer.marketplaces().iter().all(|m| m.name != "theirs"), "hidden");
    let after = std::fs::read_to_string(home.path().join("plugins/known_marketplaces.json")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn removing_a_built_in_marketplace_hides_it_and_adding_it_back_shows_it() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    let i = installer(&config);
    i.remove_marketplace(marketplace::OFFICIAL).unwrap();
    assert!(i.marketplaces().iter().all(|m| m.name != marketplace::OFFICIAL));

    mkt.write(".claude-plugin/marketplace.json", r#"{ "name": "claude-plugins-official", "plugins": [] }"#);
    i.add_marketplace(&mkt.path().display().to_string()).unwrap();
    let back = i.marketplaces().into_iter().find(|m| m.name == marketplace::OFFICIAL).unwrap();
    assert_eq!(back.origin, MarketplaceOrigin::Own);
}

#[test]
fn the_catalog_says_what_cannot_be_installed_and_what_is() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    skill(&mkt, "a", "s", "x");
    let path = market(
        &mkt,
        json!([
            { "name": "a", "source": "./a", "description": "first", "version": "1.0.0" },
            { "name": "b", "source": { "source": "npm", "package": "b" } }
        ]),
    );
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();

    let before = i.catalog("market").unwrap();
    assert_eq!(before[0].id, "a@market");
    assert_eq!(before[0].installed, None);
    assert!(before[1].unavailable.as_deref().unwrap().contains("npm"));
    assert!(i.install("b@market").is_err());

    i.install("a@market").unwrap();
    assert_eq!(i.catalog("market").unwrap()[0].installed.as_deref(), Some("1.0.0"));
}

#[test]
fn a_plugin_inside_the_marketplace_installs_enabled_and_loads() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    mkt.write("plugins/a/.claude-plugin/plugin.json", r#"{ "name": "a", "version": "2.0.0" }"#);
    skill(&mkt, "plugins/a", "review", "review it");
    let path = market(&mkt, json!([{ "name": "a", "source": "./plugins/a" }]));
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();

    let summary = i.install("a@market").unwrap();
    assert_eq!(summary.id, "a@market");
    assert!(summary.enabled, "installing is the consent");
    assert_eq!(summary.skills, 1);
    assert_eq!(summary.origin, PluginOrigin::Marketplace);

    let copy = config.path().join("plugin-cache/market/a/2.0.0");
    assert!(copy.join("skills/review/SKILL.md").is_file());
    assert!(!copy.join(".git").exists());
    let catalog = loaded(&config);
    assert!(catalog.get("a@market").unwrap().enabled);
    no_leftovers(&config);
}

#[test]
fn a_plugin_in_its_own_repository_is_fetched_at_the_pinned_commit() {
    let (config, mkt, plugin) = (TempDir::new(), TempDir::new(), TempDir::new());
    plugin.write(".claude-plugin/plugin.json", r#"{ "name": "pinned" }"#);
    skill(&plugin, ".", "s", "first");
    let plugin_path = repo(&plugin);
    let pinned = head(&plugin_path);
    skill(&plugin, ".", "s", "second");
    repo(&plugin);

    let path = market(
        &mkt,
        json!([{ "name": "pinned", "source": { "source": "url", "url": file_url(&plugin_path), "sha": pinned } }]),
    );
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();
    i.install("pinned@market").unwrap();

    let p = loaded(&config);
    let root = &p.get("pinned@market").unwrap().root;
    let text = std::fs::read_to_string(root.join("skills/s/SKILL.md")).unwrap();
    assert!(text.ends_with("first"), "{text}");
}

#[test]
fn a_plugin_in_a_subdirectory_of_a_repository_is_taken_from_there() {
    let (config, mkt, mono) = (TempDir::new(), TempDir::new(), TempDir::new());
    mono.write("packages/tool/.claude-plugin/plugin.json", r#"{ "name": "tool" }"#);
    mono.write("README.md", "the rest of the repository");
    let mono_path = repo(&mono);
    let path = market(
        &mkt,
        json!([{ "name": "tool", "source": { "source": "git-subdir", "url": file_url(&mono_path), "path": "packages/tool" } }]),
    );
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();
    i.install("tool@market").unwrap();

    let root = loaded(&config).get("tool@market").unwrap().root.clone();
    assert!(root.join(".claude-plugin/plugin.json").is_file());
    assert!(!root.join("README.md").exists(), "only the subdirectory");
}

#[test]
fn a_non_strict_entry_is_the_plugins_manifest() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    mkt.write("bare/extra/s/SKILL.md", "---\nname: s\ndescription: d\n---\n");
    let path = market(
        &mkt,
        json!([{
            "name": "bare", "source": "./bare", "strict": false, "version": "0.1.0",
            "description": "from the entry", "skills": ["./extra"],
            "mcpServers": { "srv": { "command": "node" } }
        }]),
    );
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();
    let summary = i.install("bare@market").unwrap();
    assert_eq!(summary.description.as_deref(), Some("from the entry"));
    assert_eq!((summary.skills, summary.mcp_servers), (1, 1));
}

#[test]
fn a_package_the_loader_refuses_is_not_installed_and_leaves_nothing() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    // An Agent Plugins manifest without its `$schema`: rejected whole.
    mkt.write("broken/plugin.json", r#"{ "name": "broken" }"#);
    let path = market(&mkt, json!([{ "name": "broken", "source": "./broken" }]));
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();

    let e = i.install("broken@market").unwrap_err();
    assert!(e.to_string().contains("не принят"), "{e}");
    assert!(!config.path().join("plugin-cache/market").exists());
    assert!(installed(config.path()).is_empty());
    no_leftovers(&config);
}

#[test]
fn an_update_takes_new_files_and_says_when_there_were_none() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    skill(&mkt, "a", "s", "old");
    let path = market(&mkt, json!([{ "name": "a", "source": "./a" }]));
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();
    i.install("a@market").unwrap();

    let same = i.update("a@market").unwrap();
    assert!(!same.changed, "the same files fetched again are no update");

    skill(&mkt, "a", "s", "new");
    repo(&mkt);
    let newer = i.update("a@market").unwrap();
    assert!(newer.changed);
    let root = loaded(&config).get("a@market").unwrap().root.clone();
    assert!(std::fs::read_to_string(root.join("skills/s/SKILL.md")).unwrap().ends_with("new"));
    let versions = std::fs::read_dir(config.path().join("plugin-cache/market/a")).unwrap().count();
    assert_eq!(versions, 1, "the old version is gone");
    no_leftovers(&config);
}

#[test]
fn uninstalling_removes_the_files_and_keeps_the_plugins_data() {
    let (config, mkt) = (TempDir::new(), TempDir::new());
    skill(&mkt, "a", "s", "x");
    let path = market(&mkt, json!([{ "name": "a", "source": "./a" }]));
    let i = installer(&config);
    i.add_marketplace(&file_url(&path)).unwrap();
    i.install("a@market").unwrap();
    let plugin = loaded(&config).get("a@market").unwrap().clone();
    std::fs::create_dir_all(&plugin.data).unwrap();

    i.uninstall("a@market").unwrap();
    assert!(loaded(&config).get("a@market").is_none());
    assert!(!plugin.root.exists());
    assert!(plugin.data.exists(), "what the plugin kept for the human stays");
    assert!(i.uninstall("a@market").is_err());
}

#[test]
fn a_plugin_installs_from_a_folder_by_address() {
    let (config, src) = (TempDir::new(), TempDir::new());
    src.write(".claude-plugin/plugin.json", r#"{ "name": "handmade", "version": "0.0.1" }"#);
    let summary = installer(&config).install_from(&src.path().display().to_string()).unwrap();
    assert_eq!(summary.id, "handmade@direct");
    assert!(config.path().join("plugin-cache/direct/handmade/0.0.1").is_dir());
}

#[test]
fn a_plugin_installs_from_a_git_address_and_updates_from_it() {
    let (config, src) = (TempDir::new(), TempDir::new());
    src.write(".claude-plugin/plugin.json", r#"{ "name": "remote" }"#);
    let path = repo(&src);
    let i = installer(&config);
    i.install_from(&file_url(&path)).unwrap();
    assert!(!i.update("remote@direct").unwrap().changed);

    skill(&src, ".", "s", "x");
    repo(&src);
    assert!(i.update("remote@direct").unwrap().changed);
    assert_eq!(loaded(&config).report().plugins[0].skills, 1);
}

#[test]
fn yacas_install_comes_before_the_same_plugin_installed_by_claude_code() {
    let (config, mkt, home) = (TempDir::new(), TempDir::new(), TempDir::new());
    mkt.write("p/.claude-plugin/plugin.json", r#"{ "name": "p" }"#);
    let path = market(&mkt, json!([{ "name": "p", "source": "./p" }]));
    let theirs = home.write("cache/p/.claude-plugin/plugin.json", r#"{ "name": "p" }"#);
    let theirs_root = theirs.parent().unwrap().parent().unwrap();
    home.write(
        "plugins/installed_plugins.json",
        &json!({ "version": 2, "plugins": { "p@market": [{ "scope": "user", "installPath": theirs_root }] } }).to_string(),
    );
    let i = Installer::new(config.path(), Some(home.path().to_path_buf()));
    i.add_marketplace(&file_url(&path)).unwrap();
    i.install("p@market").unwrap();

    let catalog = PluginSources::new(config.path(), Some(home.path().to_path_buf())).load();
    let p = catalog.get("p@market").unwrap();
    assert_eq!(p.origin, PluginOrigin::Marketplace);
    assert!(catalog.diagnostics().iter().any(|d| d.message.contains("уже загружен")));
}

#[test]
fn a_marketplace_address_is_a_folder_a_github_repo_or_a_git_url() {
    let dir = TempDir::new();
    assert!(matches!(MarketplaceSource::parse(&dir.path().display().to_string()), Ok(MarketplaceSource::Directory { .. })));
    assert_eq!(
        MarketplaceSource::parse("anthropics/claude-plugins-official").unwrap(),
        MarketplaceSource::Github { repo: "anthropics/claude-plugins-official".into(), reference: None }
    );
    assert!(matches!(MarketplaceSource::parse("https://example.com/m.git"), Ok(MarketplaceSource::Git { .. })));
    assert!(matches!(MarketplaceSource::parse("git@github.com:a/b.git"), Ok(MarketplaceSource::Git { .. })));
    assert!(MarketplaceSource::parse("just words").is_err());
}
