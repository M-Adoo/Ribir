//! Changelog AST manipulation and parsing.
//!
//! This module provides AST-based changelog parsing and manipulation using
//! comrak.

use std::{cell::RefCell, fs};

use comrak::{
  Arena, Node, Options,
  nodes::{
    Ast, AstNode, LineColumn, ListDelimType, ListType, NodeCode, NodeHeading, NodeList, NodeValue,
  },
  parse_document,
};
use semver::Version;

use crate::{
  types::{Highlight, Result},
  utils::get_changelog_path,
};

/// Changelog marker constants.
pub const MARKER_START: &str = "<!-- RIBIR_CHANGELOG_START -->";
pub const MARKER_END: &str = "<!-- RIBIR_CHANGELOG_END -->";

// ============================================================================
// Changelog AST Types
// ============================================================================

/// Parsed release from changelog.
pub struct Release<'a> {
  pub version: Version,
  pub date: String,
  pub header: Node<'a>,
}

/// Changelog AST wrapper.
pub struct Changelog<'a> {
  pub root: Node<'a>,
}

impl<'a> Changelog<'a> {
  pub fn analyze(root: Node<'a>) -> Self { Self { root } }

  pub fn releases(&self) -> Vec<Release<'a>> {
    self
      .root
      .children()
      .filter_map(|n| {
        if let NodeValue::Heading(ref h) = n.data.borrow().value {
          if h.level == 2 {
            return Release::parse(n);
          }
        }
        None
      })
      .collect()
  }

  pub fn latest_version(&self) -> Option<Version> {
    self
      .releases()
      .into_iter()
      .map(|r| r.version)
      .next()
  }

  /// Returns (pre-releases to merge, target release if exists)
  pub fn find_merge_candidates(&self, target: &Version) -> (Vec<Release<'a>>, Option<Node<'a>>) {
    let mut pres = Vec::new();
    let mut target_node = None;

    for r in self.releases() {
      if &r.version == target {
        target_node = Some(r.header);
      } else if is_prerelease(&r.version, target) {
        pres.push(r);
      }
    }
    (pres, target_node)
  }
}

impl<'a> Release<'a> {
  pub fn parse(node: Node<'a>) -> Option<Self> {
    let text = collect_text(node);

    let parts: Vec<&str> = text.split(" - ").collect();
    let ver_str = parts
      .first()?
      .trim()
      .trim_matches(|c| c == '[' || c == ']' || c == 'v');
    let version = Version::parse(ver_str).ok()?;
    let date = parts.get(1).unwrap_or(&"").to_string();

    Some(Self { version, date, header: node })
  }
}

/// Check if a version is a prerelease of a target version.
pub fn is_prerelease(pre: &Version, target: &Version) -> bool {
  pre.major == target.major
    && pre.minor == target.minor
    && pre.patch == target.patch
    && !pre.pre.is_empty()
}

/// Collect text content from a node.
pub fn collect_text<'a>(node: Node<'a>) -> String {
  let mut s = String::new();
  for c in node.children() {
    match &c.data.borrow().value {
      NodeValue::Text(t) => s.push_str(t),
      NodeValue::Code(NodeCode { literal: t, .. }) => s.push_str(t),
      _ => s.push_str(&collect_text(c)),
    }
  }
  s
}

// ============================================================================
// Changelog Context
// ============================================================================

/// Context for changelog AST manipulation.
pub struct ChangelogContext<'a> {
  pub arena: &'a Arena<'a>,
  pub changelog: Changelog<'a>,
  pub root: Node<'a>,
  pub path: String,
}

impl<'a> ChangelogContext<'a> {
  pub fn load(arena: &'a Arena<'a>) -> Result<Self> {
    let path = get_changelog_path()?;
    Self::load_from_path(arena, &path)
  }

  /// Load from a specific path (for dry-run simulation or explicit path
  /// control).
  pub fn load_from_path(arena: &'a Arena<'a>, path: &str) -> Result<Self> {
    println!("📂 Using changelog: {}", path);

    // Fail fast: archived changelogs must exist
    if !std::path::Path::new(path).exists() {
      return Err(
        format!(
          "Changelog not found: {}. This file should have been created by the archive step.",
          path
        )
        .into(),
      );
    }

    let content = fs::read_to_string(path)?;
    Self::load_from_content_with_path(arena, &content, path.to_string())
  }

  /// Load from content string (for testing and simulation).
  #[cfg(test)]
  pub fn load_from_content(arena: &'a Arena<'a>, content: &str) -> Result<Self> {
    Self::load_from_content_with_path(arena, content, String::new())
  }

  fn load_from_content_with_path(
    arena: &'a Arena<'a>, content: &str, path: String,
  ) -> Result<Self> {
    let root = parse_document(arena, content, &Options::default());
    let changelog = Changelog::analyze(root);
    Ok(Self { arena, changelog, root, path })
  }

  pub fn save(&self, dry_run: bool) -> Result<()> {
    self.save_and_get_content(dry_run)?;
    Ok(())
  }

  /// Save changelog and return the generated content.
  pub fn save_and_get_content(&self, dry_run: bool) -> Result<String> {
    let mut content = String::new();
    comrak::format_commonmark(self.root, &Options::default(), &mut content)?;

    if dry_run {
      println!("📝 Preview:\n{}\n... (truncated)", &content.chars().take(2000).collect::<String>());
      println!("\n💡 Run with --write to apply.");
    } else {
      // Ensure parent directory exists
      if let Some(parent) = std::path::Path::new(&self.path).parent() {
        if !parent.as_os_str().is_empty() {
          fs::create_dir_all(parent)?;
        }
      }
      fs::write(&self.path, &content)?;
      println!("✅ Saved {}", self.path);
    }
    Ok(content)
  }

  pub fn new_node(&self, value: NodeValue) -> Node<'a> {
    self
      .arena
      .alloc(AstNode::new(RefCell::new(Ast::new(value, LineColumn { line: 0, column: 0 }))))
  }

  pub fn new_text(&self, text: String) -> Node<'a> { self.new_node(NodeValue::Text(text.into())) }

  pub fn new_heading(&self, level: u8, text: &str) -> Node<'a> {
    let h = self.new_node(NodeValue::Heading(NodeHeading { level, setext: false, closed: false }));
    h.append(self.new_text(text.to_string()));
    h
  }

  pub fn deep_clone<'b>(&self, node: Node<'b>) -> Node<'a> {
    let new_node = self.new_node(node.data.borrow().value.clone());
    for child in node.children() {
      new_node.append(self.deep_clone(child));
    }
    new_node
  }

  pub fn new_list_item(&self, text: &str) -> Node<'a> {
    let item = self.new_node(NodeValue::Item(NodeList {
      list_type: ListType::Bullet,
      delimiter: ListDelimType::Period,
      bullet_char: b'-',
      tight: true,
      ..NodeList::default()
    }));

    let p = self.new_node(NodeValue::Paragraph);
    p.append(self.new_text(text.to_string()));
    item.append(p);
    item
  }

  pub fn ensure_release(&self, ver: &Version, date: &str) -> Node<'a> {
    if let Some(r) = self
      .changelog
      .releases()
      .iter()
      .find(|r| &r.version == ver)
    {
      return r.header;
    }

    // Create new
    let text = format!("[{}] - {}", ver, date);
    let h2 = self.new_heading(2, &text);

    // Insert: Find insertion point (first H2 or specific marker)
    let insert_node = self
      .root
      .children()
      .find(|n| {
        // After start marker
        if let NodeValue::HtmlBlock(ref h) = n.data.borrow().value {
          return h.literal.contains("next-header");
        }
        // Or before first H2
        matches!(n.data.borrow().value, NodeValue::Heading(ref h) if h.level == 2)
      })
      .unwrap_or(self.root.last_child().unwrap_or(self.root));

    // If we fell back to root (empty doc), append.
    // If we found a child but it's the root itself (shouldn't happen with unwrap_or
    // logic above unless empty), append. Note: comrak nodes are pointers.
    let is_root = {
      // simple check if it has no parent, root is the only node without parent
      // usually
      insert_node.parent().is_none()
    };

    if is_root {
      self.root.append(h2);
    } else if matches!(insert_node.data.borrow().value, NodeValue::HtmlBlock(_)) {
      insert_node.insert_after(h2);
    } else {
      insert_node.insert_before(h2);
    }

    h2
  }

  /// Merge prereleases into target version (shared logic for changelog and
  /// release commands).
  pub fn merge_prereleases(&self, target: &Version) -> Result<()> {
    let (mut prereleases, target_node) = self.changelog.find_merge_candidates(target);
    if prereleases.is_empty() {
      return Err(format!("No pre-releases found for {}", target).into());
    }
    println!("📦 Merging {} pre-releases", prereleases.len());

    // We will collect all content into these buckets
    let mut sections: Vec<(String, Vec<Node<'a>>)> = Vec::new();
    let mut intro: Vec<Node<'a>> = Vec::new();

    // Helper to extract content from a release
    let mut process_release = |header: Node<'a>| {
      let mut curr = header.next_sibling();
      let mut current_section_title: Option<String> = None;

      while let Some(node) = curr {
        let next = node.next_sibling();

        // Check if we hit the next release (H1/H2)
        if let NodeValue::Heading(ref h) = node.data.borrow().value {
          if h.level <= 2 {
            break;
          }
          if h.level == 3 {
            // New section found (e.g., "### Features")
            let title = collect_text(node).trim().to_string();
            current_section_title = Some(title);
            // We detach the header, we'll recreate a unified one later
            node.detach();
            curr = next;
            continue;
          }
        }

        node.detach();

        if let Some(title) = &current_section_title {
          // Add to corresponding section
          if let Some(pos) = sections.iter().position(|(t, _)| t == title) {
            sections[pos].1.push(node);
          } else {
            sections.push((title.clone(), vec![node]));
          }
        } else {
          // Content before any H3 header goes to intro
          intro.push(node);
        }

        curr = next;
      }
    };

    // 1. Process existing target content if it exists
    if let Some(h) = target_node {
      process_release(h);
    }

    // 2. Process all pre-releases
    for pre in prereleases.drain(..) {
      process_release(pre.header);
      pre.header.detach();
    }

    // 3. Rebuild the target release
    // If target didn't exist, create it (or get it again, process_release detached
    // its children but not the header)
    let target_release =
      target_node.unwrap_or_else(|| self.ensure_release(target, &crate::utils::today()));

    let mut insert_point = target_release;

    // Insert intro nodes first
    for node in intro {
      insert_point.insert_after(node);
      insert_point = node;
    }

    // Insert sections
    for (title, nodes) in sections {
      // Create new H3 header
      let h3 = self.new_heading(3, &title);
      insert_point.insert_after(h3);
      insert_point = h3;

      // Insert all nodes for this section
      for node in nodes {
        insert_point.insert_after(node);
        insert_point = node;
      }
    }

    Ok(())
  }
}

// ============================================================================
// Helper Functions
// ============================================================================

const HIGHLIGHTS_START_MARKER: &str = "<!-- HIGHLIGHTS_START -->";
const HIGHLIGHTS_END_MARKER: &str = "<!-- HIGHLIGHTS_END -->";

/// Format highlights as markdown.
pub fn format_highlights(highlights: &[Highlight]) -> String {
  let mut result = String::from("**Highlights:**\n");
  for h in highlights {
    result.push_str(&format!("- {} {}\n", h.emoji, h.description));
  }
  result
}

/// Extract highlights section from PR body (between HIGHLIGHTS_START/END
/// markers). Returns the raw highlights text for direct insertion into
/// changelog.
pub fn extract_highlights_from_pr_body(body: &str) -> Option<String> {
  let start = body.find(HIGHLIGHTS_START_MARKER)? + HIGHLIGHTS_START_MARKER.len();
  let end = body.find(HIGHLIGHTS_END_MARKER)?;

  if start >= end {
    return None;
  }

  let content = body[start..end].trim();
  if content.is_empty() { None } else { Some(content.to_string()) }
}

/// Update highlights section in PR body (between HIGHLIGHTS_START/END markers).
pub fn update_pr_body_highlights(body: &str, highlights_md: &str) -> Result<String> {
  let start = body
    .find(HIGHLIGHTS_START_MARKER)
    .ok_or("PR body missing HIGHLIGHTS_START marker")?;
  let end = body
    .find(HIGHLIGHTS_END_MARKER)
    .ok_or("PR body missing HIGHLIGHTS_END marker")?;

  if start >= end {
    return Err("Invalid highlight markers in PR body".into());
  }

  let marker_end = start + HIGHLIGHTS_START_MARKER.len();
  let mut result = String::with_capacity(body.len());
  result.push_str(&body[..marker_end]);
  result.push('\n');
  result.push_str(highlights_md);
  result.push_str(&body[end..]);

  Ok(result)
}

/// Insert highlights text into changelog after version header.
pub fn insert_highlights_text(
  changelog: &str, version: &str, highlights_text: &str,
) -> Result<String> {
  // Try both escaped and unescaped patterns (comrak may escape brackets)
  let patterns = [format!("## [{}]", version), format!("## \\[{}\\]", version)];

  let header_pos = patterns
    .iter()
    .find_map(|p| changelog.find(p))
    .ok_or("Version header not found")?;

  let rest = &changelog[header_pos..];
  let header_end = rest.find('\n').unwrap_or(rest.len());
  let insert_pos = header_pos + header_end;

  let mut result = String::with_capacity(changelog.len() + highlights_text.len() + 4);
  result.push_str(&changelog[..insert_pos]);
  result.push_str("\n\n");
  result.push_str(highlights_text);
  result.push_str(&changelog[insert_pos..]);

  Ok(result)
}

/// Extract a version section from the changelog (string-based, for output).
pub fn extract_version_section(changelog: &str, version: &str) -> Option<String> {
  // Try both escaped and unescaped patterns (comrak may escape brackets)
  let patterns = [format!("## [{}]", version), format!("## \\[{}\\]", version)];

  let (start, matched_pattern) = patterns
    .iter()
    .find_map(|p| changelog.find(p).map(|pos| (pos, p.as_str())))?;

  let rest = &changelog[start..];

  // Find the next version header (either escaped or unescaped)
  let end = rest[3..]
    .find("\n## [")
    .or_else(|| rest[3..].find("\n## \\["))
    .map(|pos| pos + 3)
    .unwrap_or(rest.len());

  let section = rest[..end].to_string();

  // Unescape the section if it was escaped
  if matched_pattern.contains("\\[") {
    Some(section.replace("\\[", "[").replace("\\]", "]"))
  } else {
    Some(section)
  }
}

/// Parse the latest version from changelog.
pub fn parse_latest_version(changelog: &str) -> Option<String> {
  let arena = Arena::new();
  let root = parse_document(&arena, changelog, &Options::default());
  let cl = Changelog::analyze(root);
  cl.latest_version().map(|v| v.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_prerelease() {
    let target = Version::parse("0.5.0").unwrap();
    assert!(is_prerelease(&Version::parse("0.5.0-alpha.1").unwrap(), &target));
    assert!(is_prerelease(&Version::parse("0.5.0-rc.1").unwrap(), &target));
    assert!(!is_prerelease(&Version::parse("0.5.0").unwrap(), &target));
    assert!(!is_prerelease(&Version::parse("0.4.0-alpha.1").unwrap(), &target));
  }

  #[test]
  fn test_parse_release_header() {
    let arena = Arena::new();
    let content = "## [0.5.0-alpha.1] - 2025-01-15\n\n### Features\n- feat: test";
    let root = parse_document(&arena, content, &Options::default());
    let changelog = Changelog::analyze(root);

    let releases = changelog.releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].version.to_string(), "0.5.0-alpha.1");
    assert_eq!(releases[0].date, "2025-01-15");
  }

  #[test]
  fn test_parse_multiple_releases() {
    let arena = Arena::new();
    let content = r#"
## [0.5.0-alpha.2] - 2025-01-20

### Features
- feat: feature 2

## [0.5.0-alpha.1] - 2025-01-15

### Features
- feat: feature 1
"#;
    let root = parse_document(&arena, content, &Options::default());
    let changelog = Changelog::analyze(root);

    let releases = changelog.releases();
    assert_eq!(releases.len(), 2);
    assert_eq!(releases[0].version.to_string(), "0.5.0-alpha.2");
    assert_eq!(releases[1].version.to_string(), "0.5.0-alpha.1");
  }

  #[test]
  fn test_find_merge_candidates() {
    let arena = Arena::new();
    let content = r#"
## [0.5.0] - 2025-02-01

### Features
- feat: stable

## [0.5.0-rc.1] - 2025-01-25

### Features
- feat: rc1

## [0.5.0-alpha.2] - 2025-01-20

### Features
- feat: alpha2

## [0.5.0-alpha.1] - 2025-01-15

### Features
- feat: alpha1
"#;
    let root = parse_document(&arena, content, &Options::default());
    let changelog = Changelog::analyze(root);

    let target = Version::parse("0.5.0").unwrap();
    let (prereleases, target_node) = changelog.find_merge_candidates(&target);

    assert_eq!(prereleases.len(), 3);
    assert!(target_node.is_some());
  }

  #[test]
  fn test_format_highlights() {
    let highlights = vec![
      Highlight { emoji: "⚡".into(), description: "50% faster rendering".into() },
      Highlight { emoji: "🎨".into(), description: "Dark mode support".into() },
    ];
    let result = format_highlights(&highlights);
    assert!(result.contains("**Highlights:**"));
    assert!(result.contains("- ⚡ 50% faster rendering"));
    assert!(result.contains("- 🎨 Dark mode support"));
  }

  #[test]
  fn test_parse_latest_version() {
    let changelog = r#"# Changelog

## [0.5.0-alpha.5] - 2025-01-15

### Features
- feat: something

## [0.5.0-alpha.4] - 2025-01-10
"#;
    let version = parse_latest_version(changelog).unwrap();
    assert_eq!(version, "0.5.0-alpha.5");
  }

  #[test]
  fn test_extract_version_section() {
    let changelog = r#"## [0.5.0-rc.1] - 2025-01-15

### Features
- feat: something new

### Fixed
- fix: some bug

## [0.4.0] - 2025-01-01

### Features
- feat: old stuff
"#;
    let section = extract_version_section(changelog, "0.5.0-rc.1").unwrap();
    assert!(section.contains("feat: something new"));
    assert!(section.contains("fix: some bug"));
    assert!(!section.contains("feat: old stuff"));
  }

  #[test]
  fn test_extract_highlights_from_pr_body() {
    let body = r#"## Release PR

Some description here.

<!-- HIGHLIGHTS_START -->
**Highlights:**
- ⚡ 50% faster rendering
- 🎨 Dark mode support
- 🐛 Fixed memory leak
<!-- HIGHLIGHTS_END -->

More stuff below.
"#;
    let highlights = extract_highlights_from_pr_body(body).unwrap();
    assert!(highlights.contains("**Highlights:**"));
    assert!(highlights.contains("⚡ 50% faster rendering"));
    assert!(highlights.contains("🎨 Dark mode support"));
    assert!(highlights.contains("🐛 Fixed memory leak"));
  }

  #[test]
  fn test_extract_highlights_from_pr_body_no_markers() {
    let body = "No markers here";
    assert!(extract_highlights_from_pr_body(body).is_none());
  }

  #[test]
  fn test_update_pr_body_highlights() {
    let body = r#"## Release PR

Some description here.

<!-- HIGHLIGHTS_START -->
Old content
<!-- HIGHLIGHTS_END -->

More stuff below.
"#;
    let new_content = "**Highlights:**\n- 🚀 New";
    let updated = update_pr_body_highlights(body, new_content).unwrap();

    assert!(updated.contains("<!-- HIGHLIGHTS_START -->"));
    assert!(updated.contains("<!-- HIGHLIGHTS_END -->"));
    assert!(updated.contains(new_content));
    assert!(!updated.contains("Old content"));
    assert!(
      extract_highlights_from_pr_body(&updated)
        .unwrap()
        .contains("🚀 New")
    );
  }

  #[test]
  fn test_merge_prereleases_with_grouping() {
    let arena = Arena::new();
    let content = r#"
## [0.5.0-rc.2] - 2025-01-25

This is an introduction in RC2.

### Features
- feat: rc2 feature

## [0.5.0-rc.1] - 2025-01-20

### Features
- feat: rc1 feature

### Fixes
- fix: rc1 fix
"#;
    let ctx = ChangelogContext::load_from_content(&arena, content).unwrap();
    let target = Version::parse("0.5.0").unwrap();

    ctx.merge_prereleases(&target).unwrap();

    // Verify content
    let mut output = String::new();
    comrak::format_commonmark(ctx.root, &Options::default(), &mut output).unwrap();

    // Verify sections are merged
    assert!(output.contains("## [0.5.0]") || output.contains("## \\[0.5.0\\]"));
    assert!(output.contains("This is an introduction in RC2."));

    // Check Features section
    let features_pos = output.find("### Features").unwrap();
    let fixes_pos = output.find("### Fixes").unwrap();
    let intro_pos = output
      .find("This is an introduction in RC2.")
      .unwrap();

    // Intro should be before Features
    assert!(intro_pos < features_pos);

    // Ensure both features are under the same header (concatenated)
    let rc2_feat_pos = output.find("feat: rc2 feature").unwrap();
    let rc1_feat_pos = output.find("feat: rc1 feature").unwrap();

    // first, rc.1 is second. So releases() order: [rc.2, rc.1].
    // So rc.2 processed first.

    assert!(features_pos < rc2_feat_pos);
    assert!(features_pos < rc1_feat_pos);
    // Fixes should be separate
    assert!(fixes_pos > rc1_feat_pos);
    assert!(output.find("fix: rc1 fix").unwrap() > fixes_pos);

    // Ensure only one Features header
    assert_eq!(output.matches("### Features").count(), 1);
  }
}
