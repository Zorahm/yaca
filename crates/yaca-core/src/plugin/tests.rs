//! Loading plugins from real directories: both formats, both sources, the
//! switches, and what their MCP servers are started with.

use std::path::Path;

use serde_json::json;

use super::*;
use crate::mcp::config::Launch;
use crate::testutil::TempDir;

const SCHEMA: &str = r#""$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json""#;
const MCP_SCHEMA: &str = r#""$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json""#;

/// A configuration directory with Yaca's own plugins under `plugins/`, and no
/// Claude Code.
fn own(config: &TempDir) -> PluginCatalog {
    PluginSources::new(config.path(), None).load()
}

fn claude_plugin(config: &TempDir, dir: &str, manifest: &str) {
    config.write(&format!("plugins/{dir}/.claude-plugin/plugin.json"), manifest);
}

fn agent_plugin(config: &TempDir, dir: &str, manifest: &str) {
    config.write(&format!("plugins/{dir}/plugin.json"), manifest);
}

fn only(catalog: &PluginCatalog) -> &Plugin {
    assert_eq!(catalog.plugins().len(), 1, "{:?}", catalog.diagnostics());
    &catalog.plugins()[0]
}

#[test]
fn a_claude_plugin_brings_its_skills_commands_servers_and_hooks() {
    let config = TempDir::new();
    claude_plugin(&config, "tools", r#"{ "name": "tools", "version": "1.2.0" }"#);
    config.write("plugins/tools/skills/review/SKILL.md", "---\nname: review\ndescription: d\n---\n");
    config.write(
        "plugins/tools/commands/lint.md",
        "---\ndescription: Lint it\n---\nRun ${CLAUDE_PLUGIN_ROOT}/lint.sh on $ARGUMENTS",
    );
    config.write(
        "plugins/tools/.mcp.json",
        r#"{ "mcpServers": { "files": { "command": "${CLAUDE_PLUGIN_ROOT}/srv", "args": ["--x"] } } }"#,
    );
    config.write(
        "plugins/tools/hooks/hooks.json",
        r#"{ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "true" }] }] } }"#,
    );

    let catalog = own(&config);
    let p = only(&catalog);
    assert_eq!((p.id.as_str(), p.name.as_str()), ("tools", "tools"));
    assert_eq!(p.format, PluginFormat::Claude);
    assert_eq!(p.origin, PluginOrigin::Yaca);
    assert!(p.enabled, "Yaca's own are on until switched off");
    assert_eq!(p.version.as_deref(), Some("1.2.0"));
    assert_eq!(p.skill_dirs.len(), 1);
    assert_eq!(p.commands[0].name, "tools:lint");
    assert_eq!(p.commands[0].description, "Lint it");
    assert!(p.commands[0].body.contains(&format!("{}/lint.sh", plain(&p.root))), "{}", p.commands[0].body);
    assert!(p.mcp_servers.contains_key("files"));
    assert_eq!(p.hooks.len(), 1);
    assert!(p.notes.is_empty(), "{:?}", p.notes);
}

#[test]
fn a_claude_manifest_adds_paths_and_inline_servers_without_losing_the_defaults() {
    let config = TempDir::new();
    claude_plugin(
        &config,
        "x",
        r#"{ "name": "x", "skills": ["./extra"], "mcpServers": { "inline": { "command": "a" } } }"#,
    );
    config.write("plugins/x/skills/a/SKILL.md", "---\ndescription: d\n---\n");
    config.write("plugins/x/extra/b/SKILL.md", "---\ndescription: d\n---\n");
    config.write("plugins/x/.mcp.json", r#"{ "mcpServers": { "file": { "command": "b" } } }"#);

    let catalog = own(&config);
    let p = only(&catalog);
    assert_eq!(p.skill_dirs.len(), 2);
    let names: Vec<_> = p.mcp_servers.keys().map(String::as_str).collect();
    assert_eq!(names, ["file", "inline"]);
}

#[test]
fn what_yaca_cannot_load_is_a_note_and_the_rest_still_loads() {
    let config = TempDir::new();
    claude_plugin(&config, "x", r#"{ "name": "x" }"#);
    config.write("plugins/x/output-styles/a.md", "style");
    config.write("plugins/x/skills/a/SKILL.md", "---\ndescription: d\n---\n");
    config.write(
        "plugins/x/hooks/hooks.json",
        r#"{ "hooks": { "Setup": [], "PreToolUse": [{ "hooks": [{ "type": "prompt", "prompt": "?" }] }] } }"#,
    );
    config.write("plugins/x/.mcp.json", r#"{ "mcpServers": { "old": { "type": "sse", "url": "https://x" } } }"#);

    let catalog = own(&config);
    let p = only(&catalog);
    assert_eq!(p.skill_dirs.len(), 1);
    assert!(p.mcp_servers.is_empty());
    let notes = p.notes.join("\n");
    for expected in ["стили вывода", "Setup", "prompt", "sse"] {
        assert!(notes.contains(expected), "{expected}: {notes}");
    }
}

#[test]
fn a_component_path_that_leaves_the_plugin_is_not_followed() {
    let config = TempDir::new();
    claude_plugin(&config, "x", r#"{ "name": "x", "skills": "../../outside" }"#);
    config.write("outside/s/SKILL.md", "---\ndescription: d\n---\n");
    let catalog = own(&config);
    let p = only(&catalog);
    assert!(p.skill_dirs.is_empty());
    assert!(p.notes[0].contains("за пределы"), "{:?}", p.notes);
}

#[test]
fn an_agent_plugin_loads_its_skills_and_servers() {
    let config = TempDir::new();
    agent_plugin(&config, "dir", &format!(r#"{{ {SCHEMA}, "name": "portable" }}"#));
    config.write("plugins/dir/skills/s/SKILL.md", "---\ndescription: d\n---\n");
    config.write("plugins/dir/bin/server", "");
    config.write(
        "plugins/dir/mcp.json",
        &format!(
            r#"{{ {MCP_SCHEMA}, "mcpServers": {{
                "local": {{ "type": "stdio", "command": "./bin/server", "args": ["${{PLUGIN_DATA}}/db"], "cwd": "./bin" }},
                "remote": {{ "type": "streamable-http", "url": "https://example.com/mcp" }}
            }} }}"#
        ),
    );

    let catalog = own(&config);
    let p = only(&catalog);
    assert_eq!(p.format, PluginFormat::AgentPlugins);
    assert_eq!(p.id, "portable", "the manifest names it, not the directory");
    assert_eq!(p.skill_dirs.len(), 1);
    let local = &p.mcp_servers["local"];
    assert_eq!(Path::new(local.command.as_deref().unwrap()), p.root.join("bin").join("server"));
    assert_eq!(local.args, ["${PLUGIN_DATA}/db"], "filled in at launch, not at load");
    assert_eq!(local.cwd.as_deref(), Some("${PLUGIN_ROOT}/bin"));
    assert_eq!(p.mcp_servers["remote"].url.as_deref(), Some("https://example.com/mcp"));
    assert!(p.notes.is_empty(), "{:?}", p.notes);
}

#[test]
fn an_agent_manifest_that_breaks_the_schema_is_rejected_whole() {
    let cases = [
        (r#"{ "name": "a" }"#.to_string(), "$schema"),
        (r#"{ "$schema": "https://agent-plugins.org/schemas/9.0.0/plugin.schema.json", "name": "a" }"#.into(), "версия"),
        (format!(r#"{{ {SCHEMA}, "name": "Bad_Name" }}"#), "имя"),
        (format!(r#"{{ {SCHEMA} }}"#), "name"),
        (format!(r#"{{ {SCHEMA}, "name": "a", "version": 1 }}"#), "version"),
        (format!(r#"{{ {SCHEMA}, "name": "a", "author": {{ "name": "x", "phone": "1" }} }}"#), "author"),
        (format!(r#"{{ {SCHEMA}, "name": "a", "keywords": "x" }}"#), "keywords"),
    ];
    for (manifest, expected) in cases {
        let config = TempDir::new();
        agent_plugin(&config, "p", &manifest);
        let catalog = own(&config);
        assert!(catalog.plugins().is_empty(), "{manifest}");
        assert!(catalog.diagnostics()[0].message.contains(expected), "{manifest}: {:?}", catalog.diagnostics());
    }
}

#[test]
fn unknown_fields_and_a_bad_extensions_value_are_reported_and_ignored() {
    let config = TempDir::new();
    agent_plugin(&config, "p", &format!(r#"{{ {SCHEMA}, "name": "a", "extensions": 3, "colour": "red" }}"#));
    let catalog = own(&config);
    let notes = only(&catalog).notes.join("\n");
    assert!(notes.contains("extensions") && notes.contains("colour"), "{notes}");
}

#[test]
fn a_bad_mcp_file_turns_off_the_servers_and_a_bad_entry_only_itself() {
    let config = TempDir::new();
    agent_plugin(&config, "p", &format!(r#"{{ {SCHEMA}, "name": "a" }}"#));
    config.write("plugins/p/mcp.json", r#"{ "mcpServers": {} }"#);
    let catalog = own(&config);
    let p = only(&catalog);
    assert!(p.notes[0].contains("$schema"), "{:?}", p.notes);

    let config = TempDir::new();
    agent_plugin(&config, "p", &format!(r#"{{ {SCHEMA}, "name": "a" }}"#));
    config.write(
        "plugins/p/mcp.json",
        &format!(
            r#"{{ {MCP_SCHEMA}, "mcpServers": {{
                "good": {{ "type": "stdio", "command": "node" }},
                "shell": {{ "type": "stdio", "command": "node server.js" }},
                "escape": {{ "type": "stdio", "command": "./../x" }},
                "env": {{ "type": "stdio", "command": "node", "env": {{ "PLUGIN_ROOT": "/" }} }},
                "mixed": {{ "type": "stdio", "command": "node", "url": "https://x" }},
                "plain": {{ "type": "streamable-http", "url": "http://example.com/mcp" }},
                "cwd": {{ "type": "stdio", "command": "node", "cwd": "/tmp" }},
                "legacy": {{ "type": "sse", "url": "https://example.com/sse" }}
            }} }}"#
        ),
    );
    let catalog = own(&config);
    let p = only(&catalog);
    assert_eq!(p.mcp_servers.keys().collect::<Vec<_>>(), ["good"]);
    assert_eq!(p.notes.len(), 7, "{:?}", p.notes);
}

#[test]
fn a_directory_without_a_manifest_is_reported() {
    let config = TempDir::new();
    config.write("plugins/empty/README.md", "x");
    let catalog = own(&config);
    assert!(catalog.plugins().is_empty());
    assert!(catalog.diagnostics()[0].message.contains("манифест"));
}

#[test]
fn the_first_plugin_of_a_name_wins() {
    let config = TempDir::new();
    claude_plugin(&config, "a-first", r#"{ "name": "same" }"#);
    claude_plugin(&config, "b-second", r#"{ "name": "same" }"#);
    let catalog = own(&config);
    assert!(only(&catalog).root.ends_with("a-first"));
    assert!(catalog.diagnostics()[0].message.contains("уже загружен"));
}

/// A Claude Code home with `installs` (id → install path, scope) and its
/// `enabledPlugins`.
fn claude_home(home: &TempDir, installs: &[(&str, &Path, &str)], enabled: serde_json::Value) {
    let plugins: serde_json::Map<String, serde_json::Value> = installs
        .iter()
        .map(|(id, path, scope)| {
            (id.to_string(), json!([{ "scope": scope, "installPath": path.display().to_string() }]))
        })
        .collect();
    home.write(
        "plugins/installed_plugins.json",
        &json!({ "version": 2, "plugins": plugins }).to_string(),
    );
    home.write("settings.json", &json!({ "enabledPlugins": enabled }).to_string());
}

#[test]
fn claude_codes_installs_load_under_its_ids_and_take_its_switches() {
    let config = TempDir::new();
    let home = TempDir::new();
    let on = home.write("cache/on/.claude-plugin/plugin.json", r#"{ "name": "on" }"#);
    let off = home.write("cache/off/.claude-plugin/plugin.json", r#"{ "name": "off" }"#);
    let project = home.write("cache/proj/.claude-plugin/plugin.json", r#"{ "name": "proj" }"#);
    let root = |p: &Path| p.parent().unwrap().parent().unwrap().to_path_buf();
    claude_home(
        &home,
        &[
            ("on@market", &root(&on), "user"),
            ("off@market", &root(&off), "user"),
            ("proj@market", &root(&project), "project"),
        ],
        json!({ "off@market": false }),
    );

    let catalog = PluginSources::new(config.path(), Some(home.path().to_path_buf())).load();
    let ids: Vec<_> = catalog.plugins().iter().map(|p| (p.id.as_str(), p.enabled)).collect();
    assert_eq!(ids, [("off@market", false), ("on@market", true)], "project installs are left out");
    assert!(catalog.plugins().iter().all(|p| p.origin == PluginOrigin::ClaudeCode));
    assert_eq!(catalog.enabled().len(), 1);
}

#[test]
fn yacas_own_switch_overrides_claude_codes() {
    let config = TempDir::new();
    let home = TempDir::new();
    let manifest = home.write("cache/p/.claude-plugin/plugin.json", r#"{ "name": "p" }"#);
    let root = manifest.parent().unwrap().parent().unwrap().to_path_buf();
    claude_home(&home, &[("p@m", &root, "user")], json!({ "p@m": false }));
    let sources = PluginSources::new(config.path(), Some(home.path().to_path_buf()));

    set_enabled(config.path(), "p@m", true).unwrap();
    assert!(sources.load().get("p@m").unwrap().enabled);
    set_enabled(config.path(), "p@m", false).unwrap();
    assert!(!sources.load().get("p@m").unwrap().enabled);
    // Claude Code's files are read, never written.
    assert!(home.path().join("settings.json").exists());
    assert!(std::fs::read_to_string(home.path().join("settings.json")).unwrap().contains("false"));
}

#[test]
fn a_claude_install_without_a_manifest_takes_the_name_it_was_installed_under() {
    let config = TempDir::new();
    let home = TempDir::new();
    let skill = home.write("cache/bare/skills/s/SKILL.md", "---\ndescription: d\n---\n");
    let root = skill.parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    claude_home(&home, &[("bare@m", &root, "user")], json!({}));

    let catalog = PluginSources::new(config.path(), Some(home.path().to_path_buf())).load();
    let p = only(&catalog);
    assert_eq!((p.id.as_str(), p.name.as_str()), ("bare@m", "bare"));
    assert_eq!(p.skill_dirs.len(), 1);
}

#[test]
fn a_plugin_server_is_named_after_its_plugin_and_started_from_its_root() {
    let config = TempDir::new();
    claude_plugin(&config, "tools", r#"{ "name": "tools" }"#);
    config.write(
        "plugins/tools/.mcp.json",
        r#"{ "mcpServers": { "files": {
            "command": "./bin/srv",
            "args": ["${CLAUDE_PLUGIN_ROOT}/conf", "${HOME_X:-fallback}"],
            "cwd": "work"
        } } }"#,
    );
    let catalog = own(&config);
    let p = only(&catalog);
    let spec = &mcp_specs(catalog.plugins())[0];
    assert_eq!(spec.name, "plugin:tools:files");
    assert_eq!(spec.source, McpSource::Plugin);

    let none = |_: &str| None;
    let Launch::Stdio { program, args, env, cwd } = spec.launch_with(Path::new("/project"), &none).unwrap() else {
        panic!("a stdio server");
    };
    assert_eq!(Path::new(&program), p.root.join("bin/srv"));
    assert_eq!(args, [format!("{}/conf", p.root.display()), "fallback".to_string()]);
    assert_eq!(cwd, p.root.join("work"));
    assert_eq!(env["CLAUDE_PLUGIN_ROOT"], p.root.display().to_string());
    assert_eq!(env["PLUGIN_DATA"], p.data.display().to_string());
    assert!(p.data.is_dir(), "created before the server starts");
}

#[test]
fn an_agent_plugin_server_leaves_other_variables_as_written() {
    let config = TempDir::new();
    agent_plugin(&config, "p", &format!(r#"{{ {SCHEMA}, "name": "a" }}"#));
    config.write(
        "plugins/p/mcp.json",
        &format!(r#"{{ {MCP_SCHEMA}, "mcpServers": {{ "s": {{ "type": "stdio", "command": "node", "args": ["${{PLUGIN_ROOT}}/x", "${{HOME}}"] }} }} }}"#),
    );
    let catalog = own(&config);
    let spec = &mcp_specs(catalog.plugins())[0];
    let home = |name: &str| (name == "HOME").then(|| "/home/me".to_string());
    let Launch::Stdio { args, .. } = spec.launch_with(Path::new("/project"), &home).unwrap() else {
        panic!("a stdio server");
    };
    assert_eq!(args[0], format!("{}/x", catalog.plugins()[0].root.display()));
    assert_eq!(args[1], "${HOME}", "Agent Plugins §9 expands only the two plugin variables");
}

#[test]
fn the_report_counts_what_each_plugin_brings() {
    let config = TempDir::new();
    claude_plugin(&config, "x", r#"{ "name": "x", "description": "d" }"#);
    config.write("plugins/x/skills/a/SKILL.md", "---\ndescription: d\n---\n");
    config.write("plugins/x/skills/b/SKILL.md", "---\ndescription: d\n---\n");
    config.write("plugins/x/commands/c.md", "c");
    let report = own(&config).report();
    let s = &report.plugins[0];
    assert_eq!((s.skills, s.commands, s.mcp_servers, s.hooks), (2, 1, 0, 0));
}
