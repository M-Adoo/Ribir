//! Release command implementations.

use std::{
  fs,
  process::{Command, Stdio},
};

use comrak::Arena;
use semver::Version;

use crate::{changelog::*, external::*, types::*, utils::*};

const HIGHLIGHTS_PROMPT: &str = r#"Analyze these changelog entries and select 3-5 highlights for a release announcement.

## Changelog Entries

{changelog_entries}

## Selection Criteria

1. **Impact** - Prioritize user-facing changes over internal refactors
2. **Newsworthy** - Features and performance improvements over minor fixes
3. **Diversity** - Cover different areas (widgets, core, performance, etc.)
4. **Clarity** - Changes that are easy to understand and explain

## Output Requirements

Generate 3-5 highlights (no more, no less) with:
- **Emoji** - Match the change type: ✨ (new), 🎨 (features), ⚡ (perf), 🐛 (fix), 📚 (docs), 💥 (breaking), 🔧 (internal)
- **Description** - Under 60 characters, user-friendly, active voice
  - Good: "50% faster WASM rendering"
  - Bad: "WASM rendering performance was improved by 50%"

## Output Format

Return ONLY valid JSON:
{"highlights": [{"emoji": "⚡", "description": "50% faster WASM rendering"}, ...]}

Example output:
{"highlights": [
  {"emoji": "⚡", "description": "50% faster WASM rendering"},
  {"emoji": "🎨", "description": "Dark mode support for all widgets"},
  {"emoji": "🔧", "description": "Plugin system for extensibility"},
  {"emoji": "🐛", "description": "Fixed memory leak in event handling"}
]}"#;

// ============================================================================
// Public API
// ============================================================================

/// Execute release command.
pub fn cmd_release(config: &Config, cmd: &ReleaseCmd) -> Result<()> {
  match cmd {
    ReleaseCmd::Next { level, .. } => cmd_release_next(config, *level),
    ReleaseCmd::EnterRc { .. } => cmd_release_enter_rc(config),
    ReleaseCmd::Publish { pr_id } => cmd_release_publish(config, pr_id.as_deref()),
    ReleaseCmd::Stable { version, .. } => cmd_release_stable(config, version.as_deref()),
    ReleaseCmd::Verify => cmd_release_verify(),
    ReleaseCmd::Highlights { context } => cmd_release_highlights(config, context.as_deref()),
    ReleaseCmd::SocialCard => cmd_release_social_card(config),
  }
}

/// Execute full release at the specified level.
pub fn cmd_release_next(config: &Config, level: ReleaseLevel) -> Result<()> {
  let level_str = level.as_str();
  println!("🚀 Starting {} release...", level_str);

  let version = get_next_version(level_str)?;
  println!("📦 Next version: {}", version);

  println!("📋 Collecting changelog entries...");
  let changelog_entries = collect_changelog_entries(&version, config.dry_run)?;

  if !config.dry_run {
    run_git(&["add", "CHANGELOG.md"])?;
    println!("🔧 Running cargo release...");
    run_cargo_release(level_str)?;
  } else {
    println!("   Would run: cargo release {} --execute --no-confirm", level_str);
  }

  let is_prerelease = matches!(level, ReleaseLevel::Alpha | ReleaseLevel::Rc);
  println!("🎉 Creating GitHub Release (prerelease: {})...", is_prerelease);

  let release_notes = get_release_notes(&version, Some(&changelog_entries))?;

  if !config.dry_run {
    create_github_release(&version, &release_notes, is_prerelease)?;
    println!("\n✅ Release {} complete!", version);
  } else {
    print_dry_run_summary(&version, &changelog_entries, &release_notes);
  }

  Ok(())
}

/// Enter RC phase: create release branch, merge changelog, generate highlights,
/// create PR, and publish RC.1. Version is auto-detected from the latest git
/// tag.
pub fn cmd_release_enter_rc(config: &Config) -> Result<()> {
  let version = detect_version_from_tag()?;
  let rc_version = format!("{}.{}.{}-rc.1", version.major, version.minor, version.patch);
  let branch_name = format!("release-{}.{}.x", version.major, version.minor);
  let archive_path = format!("changelogs/CHANGELOG-{}.{}.md", version.major, version.minor);

  println!("🚀 Entering RC phase for version {}", version);

  // Step 1: Verify environment and archive changelog on master
  verify_changelog_version(&version)?;
  println!("📦 Archiving CHANGELOG.md to {}", archive_path);
  if !config.dry_run {
    archive_changelog(&version)?;
    run_git(&["add", "CHANGELOG.md", &archive_path])?;
    run_git(&[
      "commit",
      "-m",
      &format!(
        "chore: archive changelog for v{}.{}\n\n🤖 Generated with ribir-bot",
        version.major, version.minor
      ),
    ])?;
    run_git(&["push"])?;
    println!("✅ Archived changelog committed to master");
  }

  // Step 2: Create release branch
  if branch_exists(&branch_name)? {
    return Err(
      format!("Release branch {} already exists. Cannot re-enter RC phase.", branch_name).into(),
    );
  }

  println!("🌿 Creating release branch: {}", branch_name);
  if !config.dry_run {
    create_release_branch(&version)?;
  }

  // Step 3: Merge alpha changelog entries
  println!("🔀 Merging alpha changelog entries for {}...", rc_version);
  let source_path = if config.dry_run { "CHANGELOG.md" } else { &archive_path };
  let changelog_content = run_changelog_merge(&rc_version, config.dry_run, Some(source_path))?;

  // Step 4: Generate and insert AI highlights
  if !config.dry_run {
    let (highlights, updated_changelog) = prepare_highlights(&rc_version, changelog_content)?;
    println!("📝 Generated {} highlights", highlights.len());

    fs::write(&archive_path, &updated_changelog)?;
    println!("✅ Updated {}", archive_path);

    commit_and_create_release_pr(&rc_version, &branch_name)?;

    println!("📦 Publishing {}...", rc_version);
    run_cargo_release("rc")?;

    println!("🎉 Creating GitHub Release for {}...", rc_version);
    let release_notes = extract_version_section(&updated_changelog, &rc_version)
      .ok_or_else(|| format!("Release notes not found for {}", rc_version))?;
    create_github_release(&rc_version, &release_notes, true)?;
  } else {
    println!("📝 Skipping AI highlights generation in dry-run mode");
    println!("\n💡 Run without --dry-run to apply changes.");
  }

  Ok(())
}

/// Publish GitHub release.
pub fn cmd_release_publish(config: &Config, pr_number: Option<&str>) -> Result<()> {
  let version = get_version_from_context()?;
  let ver = Version::parse(&version)?;
  let branch_name = format!("release-{}.{}.x", ver.major, ver.minor);

  println!("📦 Publishing release {}...", version);

  if !branch_exists(&branch_name)? {
    println!("🌿 Creating release branch: {}", branch_name);
    if !config.dry_run {
      create_release_branch(&ver)?;
    }
  }

  let release_notes = get_release_notes(&version, None)?;
  let is_prerelease = version.contains("-rc") || version.contains("-alpha");

  println!("🎉 Creating GitHub Release (prerelease={})...", is_prerelease);
  if !config.dry_run {
    create_github_release(&version, &release_notes, is_prerelease)?;
  }

  if let Some(pr) = pr_number {
    let comment = format!(
      "🎉 Release **v{}** has been published!\n\n\
       [View Release](https://github.com/RibirX/Ribir/releases/tag/v{})",
      version, version
    );
    if !config.dry_run {
      comment_on_pr(pr, &comment)?;
    }
    println!("💬 Commented on PR #{}", pr);
  }

  println!("✅ Release v{} published successfully!", version);
  Ok(())
}

/// Release stable version.
pub fn cmd_release_stable(config: &Config, version: Option<&str>) -> Result<()> {
  let version_str = version
    .map(String::from)
    .unwrap_or_else(|| detect_stable_version_from_branch().expect("Failed to detect version"));

  let version = Version::parse(&version_str)?;
  let rc1_version = format!("{}-rc.1", version_str);
  let changelog_path = get_changelog_path()?;

  println!("🚀 Releasing stable version {}...", version_str);

  let changelog = fs::read_to_string(&changelog_path)?;

  if find_rc_highlights(&changelog, &rc1_version).is_some() {
    println!("📋 Reusing highlights from RC.1");
  } else {
    eprintln!("⚠️  No highlights found in RC.1, proceeding without highlights");
  }

  if find_rc_versions(&changelog, &version).len() > 1 {
    println!("🔀 Found multiple RC versions, merging bug fix entries...");
    run_changelog_merge(&version_str, config.dry_run, None)?;
  }

  let changelog = fs::read_to_string(&changelog_path)?;
  let updated_changelog = replace_version_header(&changelog, &rc1_version, &version_str);

  if !config.dry_run {
    fs::write(&changelog_path, &updated_changelog)?;
    run_git(&["add", &changelog_path])?;
    println!("✅ Updated CHANGELOG.md with stable version");

    println!("📦 Running cargo release {}...", version_str);
    run_cargo_release_version(&version_str)?;
  } else {
    println!("   Would run: cargo release {} --execute --no-confirm", version_str);
  }

  let release_notes = extract_version_section(&updated_changelog, &version_str)
    .ok_or_else(|| format!("Release notes not found for version {}", version_str))?;

  println!("🎉 Creating stable GitHub Release...");
  if !config.dry_run {
    create_github_release(&version_str, &release_notes, false)?;
    println!("\n✅ Stable release {} published!", version_str);
  } else {
    println!("\n💡 This is a dry-run. Use --execute to apply changes.");
  }

  try_add_reaction(config);
  Ok(())
}

/// Regenerate highlights section in CHANGELOG.md.
pub fn cmd_release_highlights(config: &Config, context: Option<&str>) -> Result<()> {
  println!("🔄 Regenerating highlights in CHANGELOG.md...");

  let changelog_path = get_changelog_path()?;
  let changelog = fs::read_to_string(&changelog_path)?;
  let version = parse_latest_version(&changelog).ok_or("Could not find version in CHANGELOG.md")?;

  println!("📌 Found version: {}", version);

  let entries = extract_version_section(&changelog, &version)
    .ok_or_else(|| format!("No entries found for version {}", version))?;

  let highlights = generate_highlights(&entries, context)?;
  println!("📝 Generated {} highlights", highlights.len());

  let updated = insert_highlights(&changelog, &version, &highlights)?;

  if !config.dry_run {
    fs::write(&changelog_path, &updated)?;
    println!("✅ Updated {}", changelog_path);
    try_add_reaction(config);
  } else {
    println!("\n📝 Preview:\n{}", format_highlights(&highlights));
    println!("\n💡 Run without --dry-run to apply changes.");
  }

  Ok(())
}

/// Verify release state.
pub fn cmd_release_verify() -> Result<()> {
  println!("🔍 Verifying release state...\n");

  let branch = get_current_branch()?;
  println!("📍 Current branch: {}", branch);

  let tags = get_latest_tags(5)?;
  println!("\n🏷️  Recent tags:");
  for tag in &tags {
    println!("   {}", tag);
  }

  let changelog_path = get_changelog_path()?;
  println!("\n📄 Changelog path: {}", changelog_path);

  if let Ok(changelog) = fs::read_to_string(&changelog_path) {
    if let Some(latest) = parse_latest_version(&changelog) {
      println!("   Latest version: {}", latest);
    }
  }

  println!("\n🔧 Required tools:");
  for (cmd, name) in [("gh", "GitHub CLI"), ("gemini", "Gemini CLI")] {
    let status = if Command::new(cmd)
      .arg("--version")
      .output()
      .is_ok()
    {
      "✅"
    } else {
      "❌"
    };
    println!("   {} {}", status, name);
  }

  println!("\n✅ Verification complete");
  Ok(())
}

/// Stub for social card generation.
pub fn cmd_release_social_card(config: &Config) -> Result<()> {
  println!("⚠️  Social card generation is not yet implemented.");
  println!("📌 This feature is planned for future releases.");
  println!("\nSee: dev-docs/release-system/03-social-card-generation.md");

  try_add_reaction(config);
  Ok(())
}

// ============================================================================
// Internal Helpers - Version & Cargo
// ============================================================================

impl ReleaseLevel {
  fn as_str(self) -> &'static str {
    match self {
      ReleaseLevel::Alpha => "alpha",
      ReleaseLevel::Rc => "rc",
      ReleaseLevel::Patch => "patch",
      ReleaseLevel::Minor => "minor",
      ReleaseLevel::Major => "major",
    }
  }
}

/// Detect version from latest git tag (e.g., v0.4.0-alpha.54 -> 0.4.0).
fn detect_version_from_tag() -> Result<Version> {
  let output = Command::new("git")
    .args(["describe", "--tags", "--abbrev=0"])
    .output()?;

  if !output.status.success() {
    return Err("Failed to get latest git tag".into());
  }

  let tag = String::from_utf8_lossy(&output.stdout)
    .trim()
    .to_string();
  let tag = tag.strip_prefix('v').unwrap_or(&tag);

  // Extract base version: 0.4.0-alpha.54 -> 0.4.0
  let base_version = tag.split('-').next().unwrap_or(tag);

  Version::parse(base_version)
    .map_err(|_| format!("Could not parse version from tag: {}", tag).into())
}

fn get_next_version(level: &str) -> Result<String> {
  let output = Command::new("cargo")
    .args(["release", level, "--dry-run"])
    .stderr(Stdio::piped())
    .stdout(Stdio::piped())
    .output()?;

  let combined = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  for line in combined.lines() {
    if line.contains("Upgrading") && line.contains(" to ") {
      if let Some(after_to) = line.split(" to ").nth(1) {
        let version_str = after_to.split_whitespace().next().unwrap_or("");
        if Version::parse(version_str).is_ok() {
          return Ok(version_str.to_string());
        }
      }
    }
  }

  Err(
    format!(
      "Could not parse version from cargo release output:\n{}",
      &combined[..combined.len().min(500)]
    )
    .into(),
  )
}

fn run_cargo_release(level: &str) -> Result<()> {
  let status = Command::new("cargo")
    // TODO: Add back "--execute" after testing
    .args(["release", level, "--no-confirm"])
    .status()?;

  if !status.success() {
    return Err(format!("cargo release failed with exit code: {:?}", status.code()).into());
  }
  Ok(())
}

fn run_cargo_release_version(version: &str) -> Result<()> {
  let status = Command::new("cargo")
    // TODO: Add back "--execute" after testing
    .args(["release", version, "--no-confirm"])
    .status()?;

  if !status.success() {
    return Err(format!("cargo release failed with exit code: {:?}", status.code()).into());
  }
  Ok(())
}

fn get_version_from_context() -> Result<String> {
  // Try git tag first (most reliable after cargo release)
  if let Ok(output) = Command::new("git")
    .args(["describe", "--tags", "--abbrev=0"])
    .output()
  {
    if output.status.success() {
      let tag = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();
      if let Some(version) = tag.strip_prefix('v') {
        return Ok(version.to_string());
      }
    }
  }

  // Fallback: parse from CHANGELOG.md
  let changelog = fs::read_to_string("CHANGELOG.md")?;
  parse_latest_version(&changelog).ok_or("Could not determine version from context".into())
}

fn detect_stable_version_from_branch() -> Result<String> {
  let branch = get_current_branch()?;

  if let Some(suffix) = branch.strip_prefix("release-") {
    let parts: Vec<&str> = suffix.split('.').collect();
    if parts.len() == 3 && parts[2] == "x" {
      if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
        let version = format!("{}.{}.0", major, minor);
        println!("📌 Auto-detected version {} from branch {}", version, branch);
        return Ok(version);
      }
    }
  }

  Err(
    format!(
      "Cannot auto-detect version: current branch '{}' is not a release branch (expected \
       release-X.Y.x)",
      branch
    )
    .into(),
  )
}

// ============================================================================
// Internal Helpers - Changelog
// ============================================================================

fn collect_changelog_entries(version: &str, dry_run: bool) -> Result<String> {
  use crate::commands::cmd_collect;

  let collect_config = Config {
    command: crate::types::Cmd::Release { cmd: ReleaseCmd::Verify },
    dry_run,
    comment_id: None,
  };

  let generated_content = cmd_collect(&collect_config, version, !dry_run)?;

  if dry_run && !generated_content.is_empty() {
    Ok(
      extract_version_section(&generated_content, version)
        .unwrap_or_else(|| format!("(Changelog entries for {} will be collected)", version)),
    )
  } else {
    let changelog = fs::read_to_string("CHANGELOG.md")?;
    Ok(
      extract_version_section(&changelog, version)
        .unwrap_or_else(|| format!("(Changelog entries for {} will be collected)", version)),
    )
  }
}

fn get_release_notes(version: &str, fallback: Option<&str>) -> Result<String> {
  let changelog = fs::read_to_string("CHANGELOG.md")?;

  extract_version_section(&changelog, version)
    .or_else(|| fallback.map(String::from))
    .ok_or_else(|| format!("Release notes not found for version {}", version).into())
}

/// Verify that the current environment is correct for entering RC phase.
/// The CHANGELOG.md should contain entries for the same major.minor version.
fn verify_changelog_version(version: &Version) -> Result<()> {
  let changelog = fs::read_to_string("CHANGELOG.md")?;

  let changelog_version = parse_latest_version(&changelog)
    .and_then(|v| Version::parse(&v).ok())
    .ok_or("Could not parse version from CHANGELOG.md")?;

  if version.major != changelog_version.major || version.minor != changelog_version.minor {
    return Err(
      format!(
        "Version mismatch: git tag indicates {}.{}.x but CHANGELOG.md contains {}.{}.x",
        version.major, version.minor, changelog_version.major, changelog_version.minor
      )
      .into(),
    );
  }

  Ok(())
}

fn archive_changelog(version: &Version) -> Result<()> {
  let source = "CHANGELOG.md";
  let dest = format!("changelogs/CHANGELOG-{}.{}.md", version.major, version.minor);

  fs::create_dir_all("changelogs")?;
  fs::copy(source, &dest)?;

  let prev_minor = version.minor.saturating_sub(1);
  let new_content = format!(
    "# Changelog\n\nAll notable changes to this project will be documented in this file.\n\nFor \
     older versions:\n- [{}.{}.x changelog](changelogs/CHANGELOG-{}.{}.md)\n\n<!-- next-header \
     -->\n",
    version.major, prev_minor, version.major, prev_minor
  );

  fs::write(source, new_content)?;
  Ok(())
}

fn run_changelog_merge(
  version: &str, dry_run: bool, changelog_path: Option<&str>,
) -> Result<String> {
  let arena = Arena::new();
  let ctx = match changelog_path {
    Some(path) => ChangelogContext::load_from_path(&arena, path)?,
    None => ChangelogContext::load(&arena)?,
  };
  let target_ver = Version::parse(version)?;

  ctx.merge_prereleases(&target_ver)?;
  ctx.save_and_get_content(dry_run)
}

// ============================================================================
// Internal Helpers - Highlights
// ============================================================================

fn prepare_highlights(version: &str, changelog: String) -> Result<(Vec<Highlight>, String)> {
  println!("✨ Generating highlights with AI...");

  let entries = extract_version_section(&changelog, version)
    .ok_or_else(|| format!("No entries found for version {}", version))?;

  let highlights = generate_highlights(&entries, None)?;
  let updated_changelog = insert_highlights(&changelog, version, &highlights)?;

  Ok((highlights, updated_changelog))
}

fn generate_highlights(entries: &str, context: Option<&str>) -> Result<Vec<Highlight>> {
  let mut prompt = HIGHLIGHTS_PROMPT.replace("{changelog_entries}", entries);

  if let Some(ctx) = context {
    prompt = format!(
      "ADDITIONAL CONTEXT FROM USER:\n{}\n\nPlease consider this context when selecting and \
       writing highlights.\n\n{}",
      ctx, prompt
    );
  }

  let response = call_gemini_with_fallback(&prompt)?;
  let json_str = extract_json(&response).ok_or("No JSON found in AI response")?;

  let parsed: HighlightsResponse = serde_json::from_str(&json_str)
    .map_err(|e| format!("Invalid JSON from AI: {e}\nRaw: {response}"))?;

  validate_highlights(&parsed.highlights)?;
  Ok(parsed.highlights)
}

fn validate_highlights(highlights: &[Highlight]) -> Result<()> {
  if !(3..=5).contains(&highlights.len()) {
    return Err(
      format!("Expected 3-5 highlights, got {}. Please regenerate.", highlights.len()).into(),
    );
  }

  for h in highlights {
    if h.description.len() > 60 {
      eprintln!("⚠️  Highlight too long ({}): {}", h.description.len(), h.description);
    }
  }

  Ok(())
}

// ============================================================================
// Internal Helpers - Git & PR
// ============================================================================

fn commit_and_create_release_pr(rc_version: &str, branch_name: &str) -> Result<()> {
  let changelog_path = get_changelog_path()?;
  run_git(&["add", &changelog_path])?;

  run_git(&[
    "commit",
    "-m",
    &format!(
      "chore(release): prepare {}\n\n🤖 Generated with ribir-bot\n\nCo-Authored-By: Claude \
       <noreply@anthropic.com>",
      rc_version
    ),
  ])?;

  run_git(&["push", "-u", "origin", branch_name])?;

  let pr_title = format!("Release {} Preparation", rc_version);
  let pr_body = format!(
    "## Release Preparation for {}\n\nThis PR prepares the release materials:\n\n- Merged \
     changelog from all alpha versions\n- AI-generated highlights section\n\n**Review \
     Checklist:**\n- [ ] Verify highlights are accurate and well-written\n- [ ] Check all \
     important PRs are included\n- [ ] Confirm version and date are correct\n\n---\n🤖 Generated \
     by ribir-bot",
    rc_version
  );

  let pr_url = create_pr(&pr_title, &pr_body, "master", branch_name)?;
  println!("✅ Created PR: {}", pr_url);

  Ok(())
}

// ============================================================================
// Internal Helpers - Misc
// ============================================================================

fn print_dry_run_summary(version: &str, entries: &str, notes: &str) {
  let separator = "─".repeat(60);
  println!("\n{}", separator);
  println!("📝 Changelog entries for {}:\n", version);
  println!("{}", entries);
  println!("\n{}", separator);
  println!("📄 Release notes preview:\n");
  println!("{}", notes);
  println!("\n{}", separator);
  println!("\n💡 This is a dry-run. Use --execute to apply changes.");
}

fn try_add_reaction(config: &Config) {
  if let Some(comment_id) = config.comment_id.flatten() {
    if let Err(e) = add_reaction(comment_id, "rocket") {
      eprintln!("⚠️ Failed to add reaction: {e}");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_highlights_response() {
    let json = r#"{"highlights": [
      {"emoji": "⚡", "description": "Faster rendering"},
      {"emoji": "🐛", "description": "Fixed memory leak"},
      {"emoji": "🎨", "description": "New widgets"}
    ]}"#;
    let response: HighlightsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.highlights.len(), 3);
    assert_eq!(response.highlights[0].emoji, "⚡");
  }

  #[test]
  fn test_highlights_validation_count() {
    let too_few = vec![Highlight { emoji: "⚡".into(), description: "x".into() }];
    assert!(validate_highlights(&too_few).is_err());

    let valid = vec![
      Highlight { emoji: "⚡".into(), description: "x".into() },
      Highlight { emoji: "🎨".into(), description: "y".into() },
      Highlight { emoji: "🐛".into(), description: "z".into() },
    ];
    assert!(validate_highlights(&valid).is_ok());

    let too_many = vec![
      Highlight { emoji: "1".into(), description: "a".into() },
      Highlight { emoji: "2".into(), description: "b".into() },
      Highlight { emoji: "3".into(), description: "c".into() },
      Highlight { emoji: "4".into(), description: "d".into() },
      Highlight { emoji: "5".into(), description: "e".into() },
      Highlight { emoji: "6".into(), description: "f".into() },
    ];
    assert!(validate_highlights(&too_many).is_err());
  }
}
