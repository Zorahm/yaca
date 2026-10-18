//! The scan over real directories: precedence, leniency, the reports.

use super::*;
use crate::testutil::TempDir;

fn write_skill(dir: &TempDir, rel: &str, text: &str) {
    dir.write(&format!("{rel}/SKILL.md"), text);
}

fn ok(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\nbody of {name}\n")
}

fn names(catalog: &Catalog) -> Vec<&str> {
    catalog.skills().iter().map(|s| s.name.as_str()).collect()
}

fn project_only(dir: &TempDir) -> Catalog {
    SkillRoots::new(dir.path(), Vec::new()).scan()
}

#[test]
fn a_project_without_skills_has_no_listing_and_no_reports() {
    let dir = TempDir::new();
    let catalog = project_only(&dir);
    assert_eq!(catalog.listing(), None);
    assert!(catalog.diagnostics().is_empty());
}

#[test]
fn every_project_directory_is_read_and_the_first_of_a_name_wins() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/release", &ok("release", "from yaca"));
    write_skill(&dir, ".agents/skills/release", &ok("release", "from agents"));
    write_skill(&dir, ".agents/skills/audit", &ok("audit", "audit"));
    write_skill(&dir, ".claude/skills/docs", &ok("docs", "docs"));

    let catalog = project_only(&dir);
    assert_eq!(names(&catalog), ["audit", "docs", "release"]);
    assert_eq!(catalog.get("release").unwrap().description, "from yaca");
    let shadowed = &catalog.diagnostics()[0];
    assert!(shadowed.skipped);
    assert!(shadowed.path.contains(".agents/skills/release"), "{shadowed:?}");
    assert!(shadowed.message.contains("shadowed"), "{shadowed:?}");
}

#[test]
fn project_skills_shadow_the_humans_and_come_first_in_the_catalog() {
    let dir = TempDir::new();
    let user = TempDir::new();
    write_skill(&dir, ".yaca/skills/release", &ok("release", "project"));
    write_skill(&user, "release", &ok("release", "mine"));
    write_skill(&user, "aaa", &ok("aaa", "mine too"));

    let catalog = SkillRoots::new(dir.path(), vec![user.path().to_path_buf()]).scan();
    assert_eq!(names(&catalog), ["release", "aaa"]);
    assert_eq!(catalog.get("release").unwrap().scope, SkillScope::Project);
    assert_eq!(catalog.get("aaa").unwrap().scope, SkillScope::User);
}

#[test]
fn one_directory_reached_by_two_roots_is_one_skill_and_no_collision() {
    let dir = TempDir::new();
    let user = TempDir::new();
    write_skill(&user, "notes", &ok("notes", "n"));
    let roots = vec![user.path().to_path_buf(), user.path().to_path_buf()];

    let catalog = SkillRoots::new(dir.path(), roots).scan();
    assert_eq!(names(&catalog), ["notes"]);
    assert!(catalog.diagnostics().is_empty(), "{:?}", catalog.diagnostics());
}

#[test]
fn a_skill_without_a_description_is_left_out_and_reported() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/a", "---\nname: a\n---\nbody");
    write_skill(&dir, ".yaca/skills/b", "no header at all");
    write_skill(&dir, ".yaca/skills/c", "---\nname: [c\n---\nbody");

    let catalog = project_only(&dir);
    assert!(catalog.skills().is_empty());
    assert_eq!(catalog.diagnostics().len(), 3);
    assert!(catalog.diagnostics().iter().all(|d| d.skipped));
}

#[test]
fn cosmetic_problems_are_reported_and_the_skill_loads() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/release", &ok("Release_Tool", "d"));

    let catalog = project_only(&dir);
    // The header names the skill, not the directory.
    assert_eq!(names(&catalog), ["Release_Tool"]);
    let messages: Vec<&str> = catalog.diagnostics().iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(catalog.diagnostics().iter().all(|d| !d.skipped));
}

#[test]
fn a_name_that_cannot_be_typed_is_left_out() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/x", &ok("\"a b\"", "d"));
    let catalog = project_only(&dir);
    assert!(catalog.skills().is_empty());
    assert!(catalog.diagnostics()[0].skipped);
}

#[test]
fn a_header_without_a_name_takes_the_directorys() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/audit", "---\ndescription: d\n---\nbody");
    assert_eq!(names(&project_only(&dir)), ["audit"]);
    assert!(project_only(&dir).diagnostics().is_empty());
}

#[test]
fn directories_without_a_skill_file_and_hidden_ones_are_not_skills() {
    let dir = TempDir::new();
    dir.write(".yaca/skills/empty/README.md", "x");
    write_skill(&dir, ".yaca/skills/.hidden", &ok("hidden", "d"));
    dir.write(".yaca/skills/README.md", "x");
    let catalog = project_only(&dir);
    assert!(catalog.skills().is_empty());
    assert!(catalog.diagnostics().is_empty());
}

#[test]
fn the_listing_leaves_out_skills_kept_from_the_model() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/a", &ok("a", "shown"));
    write_skill(
        &dir,
        ".yaca/skills/b",
        "---\nname: b\ndescription: hidden\ndisable-model-invocation: true\n---\n",
    );
    let listing = project_only(&dir).listing().unwrap();
    let listed: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(listed, ["a"]);
}

#[test]
fn the_listing_is_capped_says_so_and_is_the_same_every_time() {
    let dir = TempDir::new();
    let long = "o".repeat(900);
    for n in 0..40 {
        let name = format!("skill-{n:02}");
        write_skill(&dir, &format!(".yaca/skills/{name}"), &ok(&name, &long));
    }
    let listing = project_only(&dir).listing().unwrap();
    assert!(listing.truncated);
    let bytes: usize = listing.entries.iter().map(|e| e.line().len() + 1).sum();
    assert!(bytes <= catalog::MAX_LIST_BYTES, "{bytes}");
    assert_eq!(listing.entries[0].name, "skill-00");
    // The prompt is the cached prefix: the same files must give the same list.
    assert_eq!(project_only(&dir).listing().unwrap(), listing);
}

#[test]
fn a_description_past_the_limit_is_cut_with_a_warning() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/x", &ok("x", &"a".repeat(1500)));
    let catalog = project_only(&dir);
    let description = &catalog.get("x").unwrap().description;
    assert_eq!(description.chars().count(), catalog::MAX_DESCRIPTION_CHARS);
    assert!(description.ends_with('…'));
    assert!(!catalog.diagnostics()[0].skipped);
}

#[test]
fn the_report_gives_each_skill_its_file_and_scope() {
    let dir = TempDir::new();
    write_skill(&dir, ".yaca/skills/a", &ok("a", "d"));
    let report = project_only(&dir).report();
    assert_eq!(report.skills.len(), 1);
    assert!(report.skills[0].path.ends_with(".yaca/skills/a/SKILL.md"), "{:?}", report.skills[0]);
    assert_eq!(report.skills[0].scope, SkillScope::Project);
}

#[cfg(unix)]
#[test]
fn a_project_skill_that_links_out_of_the_project_is_not_the_projects() {
    let dir = TempDir::new();
    let outside = TempDir::new();
    write_skill(&outside, "linked", &ok("linked", "d"));
    std::fs::create_dir_all(dir.path().join(".yaca/skills")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("linked"), dir.path().join(".yaca/skills/linked"))
        .unwrap();

    let catalog = project_only(&dir);
    assert!(catalog.skills().is_empty());
    assert!(catalog.diagnostics()[0].message.contains("leads outside project"));
}

#[test]
fn the_humans_directories_are_yacas_own_then_the_shared_ones() {
    let dirs = user_dirs(std::path::Path::new("/cfg"));
    assert_eq!(dirs[0], std::path::Path::new("/cfg").join("skills"));
    if dirs.len() == 3 {
        assert!(dirs[1].ends_with(".agents/skills"));
        assert!(dirs[2].ends_with(".claude/skills"));
    }
}
