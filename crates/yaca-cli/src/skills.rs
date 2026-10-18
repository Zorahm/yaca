//! `yaca skills`: the skills a session in this project would see.
//!
//! The same scan a session makes, printed: what loaded, from where, and why a
//! skill that was expected is not there. The scan is lenient, so a skill with a
//! cosmetic problem loads with a warning; this is where the warning is read.

use std::path::Path;

use anyhow::Result;
use yaca_core::provider::registry;
use yaca_core::skills::{self, SkillRoots, SkillScope};

pub fn run(project: &Path) -> Result<()> {
    let config_dir = registry::config_dir()?;
    let plugins = yaca_core::plugin::PluginSources::standard(&config_dir).load().enabled();
    let report = SkillRoots::new(project, skills::user_dirs(&config_dir))
        .with_plugins(&plugins)
        .scan()
        .report();

    if report.skills.is_empty() {
        println!("No skills.");
    }
    for skill in &report.skills {
        let scope = match skill.scope {
            SkillScope::Project => "project",
            SkillScope::User => "yours",
            SkillScope::Plugin => "plugin",
        };
        let mut marks = Vec::new();
        if !skill.model_invocable {
            marks.push("/ only");
        }
        if !skill.user_invocable {
            marks.push("not in the menu");
        }
        let marks = if marks.is_empty() {
            String::new()
        } else {
            format!(" [{}]", marks.join(", "))
        };
        println!("{} ({scope}){marks}", skill.name);
        println!("  {}", skill.description);
        println!("  {}", skill.path);
    }

    if !report.diagnostics.is_empty() {
        println!();
        for d in &report.diagnostics {
            let what = if d.skipped { "skipped" } else { "warning" };
            println!("{what}: {} — {}", d.path, d.message);
        }
    }
    Ok(())
}
