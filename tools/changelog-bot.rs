#!/usr/bin/env -S cargo +nightly -Zscript
---
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
semver = "1.0"
comrak = "0.33"
---

//! Changelog Bot - Structured changelog management
//!
//! Parses CHANGELOG.md into structured data, modifies it, and regenerates.

use std::{cell::RefCell, collections::HashMap, error::Error, fs, process::Command};

use comrak::{
  Arena, Options,
  nodes::{Ast, AstNode, LineColumn, NodeHeading, NodeList, NodeValue},
  parse_document,
};
use semver::Version;
use serde::Deserialize;

// ============================================================================
// Configuration
// ============================================================================

const CHANGELOG_PATH: &str = "CHANGELOG.md";
const MARKER_START: &str = "<!-- RIBIR_CHANGELOG_START -->";
const MARKER_END: &str = "<!-- RIBIR_CHANGELOG_END -->";

const HELP: &str = r#"
Changelog Bot - Structured changelog management

USAGE:
    ./changelog-bot.rs <COMMAND> [OPTIONS]

COMMANDS:
    collect     Collect entries from merged PRs and add new release
    merge       Merge pre-release versions into a final release
    verify      Parse and regenerate to verify logic

OPTIONS:
    --write         Apply changes (default: dry-run)
    --version VER   Target version (required for collect/merge)
    --date DATE     Release date (default: today)
    -h, --help      Show this help
"#;

fn main() -> Result<(), Box<dyn Error>> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
    println!("{}", HELP.trim());
    return Ok(());
  }

  let dry_run = !args.iter().any(|a| a == "--write");
  match args[1].as_str() {
    "verify" => cmd_verify(dry_run),
    "collect" => cmd_collect(
      get_arg(&args, "--version").ok_or("Missing --version")?,
      get_arg(&args, "--date"),
      dry_run,
    ),
    "merge" => cmd_merge(get_arg(&args, "--version").ok_or("Missing --version")?, dry_run),
    cmd => Err(format!("Unknown command: {}", cmd).into()),
  }
}

// ============================================================================
// Commands
// ============================================================================

fn cmd_collect(version: &str, date: Option<&str>, dry_run: bool) -> Result<(), Box<dyn Error>> {
  println!("📋 Collecting PRs for version {}...", version);
  let target_ver = Version::parse(version)?;
  let arena = Arena::new();
  let ctx = Context::load(&arena)?;

  let latest = ctx
    .changelog
    .latest_version()
    .ok_or("No releases found")?;
  println!("📌 Latest version: {}", latest);

  let prs = git::get_merged_prs_since(&latest)?;
  if prs.is_empty() {
    println!("✅ No new content.");
    return Ok(());
  }
  println!("🔍 Found {} new PRs", prs.len());

  let release_node = ctx.ensure_release(&target_ver, date.unwrap_or(&today()));
  let mut current_pos = release_node;

  // Group entries by type
  let mut sections: HashMap<SectionKind, Vec<&AstNode>> = HashMap::new();
  for pr in &prs {
    for (kind, entry) in extract_change_entries(&ctx, pr) {
      sections.entry(kind).or_default().push(entry);
    }
  }

  // Insert sections
  for kind in SectionKind::ALL {
    if let Some(entries) = sections.get(kind) {
      // 1. Heading
      let h3 = ctx.new_heading(3, &kind.header());
      current_pos.insert_after(h3);
      current_pos = h3;

      // 2. List
      let list = ctx.new_node(NodeValue::List(NodeList {
        list_type: comrak::nodes::ListType::Bullet,
        delimiter: comrak::nodes::ListDelimType::Period,
        bullet_char: b'-',
        tight: true,
        ..NodeList::default()
      }));
      current_pos.insert_after(list);
      current_pos = list;

      // 3. Items
      for entry in entries {
        list.append(entry);
      }
    }
  }

  ctx.save(dry_run)
}

fn cmd_merge(version: &str, dry_run: bool) -> Result<(), Box<dyn Error>> {
  println!("🔀 Merging pre-releases for {}...", version);
  let target_ver = Version::parse(version)?;
  let arena = Arena::new();
  let ctx = Context::load(&arena)?;

  let (mut prereleases, target_node) = ctx.changelog.find_merge_candidates(&target_ver);
  if prereleases.is_empty() {
    return Err(format!("No pre-releases found for {}", version).into());
  }
  println!("📦 Merging {} pre-releases", prereleases.len());

  let target_release = target_node.unwrap_or_else(|| ctx.ensure_release(&target_ver, &today()));
  let mut insert_point = target_release;

  // Move content from prereleases to target
  for pre in prereleases.drain(..) {
    // Take all siblings until the next h2
    let mut curr = pre.header.next_sibling();
    while let Some(node) = curr {
      let next = node.next_sibling();
      if matches!(node.data.borrow().value, NodeValue::Heading(h) if h.level <= 2) {
        break;
      }
      node.detach();
      insert_point.insert_after(node);
      insert_point = node;
      curr = next;
    }
    // Remove the empty prerelease header
    pre.header.detach();
  }

  ctx.save(dry_run)
}

fn cmd_verify(dry_run: bool) -> Result<(), Box<dyn Error>> {
  println!("🔍 Verifying CHANGELOG.md parsing...");
  let arena = Arena::new();
  let ctx = Context::load(&arena)?;
  let releases = ctx.changelog.releases();

  println!("\n📊 Parsed {} releases:", releases.len());
  for (i, r) in releases.iter().take(5).enumerate() {
    println!("  {}. [{}] - {}", i + 1, r.version, r.date);
  }
  ctx.save(dry_run)
}

// ============================================================================
// Data Model & Parsing
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum SectionKind {
  Features,
  Fixed,
  Changed,
  Performance,
  Documentation,
  Breaking,
  Internal,
}

impl SectionKind {
  const ALL: &'static [SectionKind] = &[
    Self::Features,
    Self::Fixed,
    Self::Changed,
    Self::Performance,
    Self::Documentation,
    Self::Breaking,
    Self::Internal,
  ];

  fn from_str(s: &str) -> Option<Self> {
    match s.trim().to_lowercase().as_str() {
      "feat" | "feature" | "features" => Some(Self::Features),
      "fix" | "fixed" => Some(Self::Fixed),
      "change" | "changed" => Some(Self::Changed),
      "perf" | "performance" => Some(Self::Performance),
      "docs" | "doc" | "documentation" => Some(Self::Documentation),
      "breaking" | "break" => Some(Self::Breaking),
      "internal" | "chore" | "refactor" | "other" => Some(Self::Internal),
      _ => None,
    }
  }

  fn header(&self) -> String {
    let (emoji, name) = match self {
      Self::Features => ("🎨", "Features"),
      Self::Fixed => ("🐛", "Fixed"),
      Self::Changed => ("🔄", "Changed"),
      Self::Performance => ("⚡", "Performance"),
      Self::Documentation => ("📚", "Documentation"),
      Self::Breaking => ("💥", "Breaking"),
      Self::Internal => ("🔧", "Internal"),
    };
    format!("### {} {}", emoji, name)
  }
}

struct Context<'a> {
  arena: &'a Arena<AstNode<'a>>,
  changelog: Changelog<'a>,
  root: &'a AstNode<'a>,
}

impl<'a> Context<'a> {
  fn load(arena: &'a Arena<AstNode<'a>>) -> Result<Self, Box<dyn Error>> {
    let content = fs::read_to_string(CHANGELOG_PATH)?;
    let root = parse_document(arena, &content, &Options::default());
    let changelog = Changelog::analyze(root);
    Ok(Self { arena, changelog, root })
  }

  fn save(&self, dry_run: bool) -> Result<(), Box<dyn Error>> {
    let mut out = Vec::new();
    comrak::format_commonmark(self.root, &Options::default(), &mut out)?;
    let content = String::from_utf8(out)?;

    if dry_run {
      println!("📝 Preview:\n{}\n... (truncated)", &content.chars().take(2000).collect::<String>());
      println!("\n💡 Run with --write to apply.");
    } else {
      fs::write(CHANGELOG_PATH, &content)?;
      println!("✅ Saved {}", CHANGELOG_PATH);
    }
    Ok(())
  }

  fn new_node(&self, value: NodeValue) -> &'a AstNode<'a> {
    self
      .arena
      .alloc(AstNode::new(RefCell::new(Ast::new(value, LineColumn { line: 0, column: 0 }))))
  }

  fn new_text(&self, text: String) -> &'a AstNode<'a> { self.new_node(NodeValue::Text(text)) }

  fn new_heading(&self, level: u8, text: &str) -> &'a AstNode<'a> {
    let h = self.new_node(NodeValue::Heading(NodeHeading { level, setext: false }));
    h.append(self.new_text(text.to_string()));
    h
  }

  fn deep_clone<'b>(&self, node: &'b AstNode<'b>) -> &'a AstNode<'a> {
    let new_node = self.new_node(node.data.borrow().value.clone());
    for child in node.children() {
      new_node.append(self.deep_clone(child));
    }
    new_node
  }

  fn new_list_item(&self, text: &str) -> &'a AstNode<'a> {
    let item = self.new_node(NodeValue::Item(NodeList {
      list_type: comrak::nodes::ListType::Bullet,
      delimiter: comrak::nodes::ListDelimType::Period,
      bullet_char: b'-',
      tight: true,
      ..NodeList::default()
    }));

    // Parse text as markdown to support links/bold etc even in simple text
    let p = self.new_node(NodeValue::Paragraph);
    p.append(self.new_text(text.to_string()));
    item.append(p);
    item
  }
}

struct Release<'a> {
  version: Version,
  date: String,
  header: &'a AstNode<'a>,
}

struct Changelog<'a> {
  root: &'a AstNode<'a>,
}

impl<'a> Changelog<'a> {
  fn analyze(root: &'a AstNode<'a>) -> Self { Self { root } }

  fn releases(&self) -> Vec<Release<'a>> {
    self
      .root
      .children()
      .filter_map(|n| {
        if let NodeValue::Heading(h) = &n.data.borrow().value {
          if h.level == 2 {
            return Release::parse(n);
          }
        }
        None
      })
      .collect()
  }

  // Removed ensure_release from Changelog as it required Context/Arena and is
  // handled by Context now.

  fn latest_version(&self) -> Option<Version> {
    self
      .releases()
      .into_iter()
      .map(|r| r.version)
      .next()
  }

  /// Returns (pre-releases to merge, target release if exists)
  fn find_merge_candidates(&self, target: &Version) -> (Vec<Release<'a>>, Option<&'a AstNode<'a>>) {
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

// Extension to Context for logic that needs Arena + Changelog
impl<'a> Context<'a> {
  fn ensure_release(&self, ver: &Version, date: &str) -> &'a AstNode<'a> {
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
        if let NodeValue::HtmlBlock(h) = &n.data.borrow().value {
          return h.literal.contains("next-header");
        }
        // Or before first H2
        matches!(&n.data.borrow().value, NodeValue::Heading(h) if h.level == 2)
      })
      .unwrap_or(self.root.last_child().unwrap_or(self.root));

    // If we found a marker or H2, insert before it?
    // Actually standard is insert after the "next-header" marker, or at top of
    // releases. Existing logic was "insert_before" if it's an H2.

    if matches!(insert_node.data.borrow().value, NodeValue::HtmlBlock(_)) {
      insert_node.insert_after(h2);
    } else {
      insert_node.insert_before(h2);
    }

    h2
  }
}

impl<'a> Release<'a> {
  fn parse(node: &'a AstNode<'a>) -> Option<Self> {
    let text = collect_text(node);
    if text.to_lowercase().contains("unreleased") {
      return None;
    }

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

fn is_prerelease(pre: &Version, target: &Version) -> bool {
  pre.major == target.major
    && pre.minor == target.minor
    && pre.patch == target.patch
    && !pre.pre.is_empty()
}

// ============================================================================
// PR Processing
// ============================================================================

#[derive(Deserialize)]
struct PR {
  number: u32,
  title: String,
  body: Option<String>,
  author: Author,
  merged_at: Option<String>,
}
#[derive(Deserialize)]
struct Author {
  login: String,
}

fn extract_change_entries<'a>(ctx: &Context<'a>, pr: &PR) -> Vec<(SectionKind, &'a AstNode<'a>)> {
  let mut entries = Vec::new();

  // 1. Try parse body block
  if let Some(body) = &pr.body {
    if body
      .to_lowercase()
      .contains("[x] no changelog needed")
    {
      return vec![];
    }

    if let Some(content) = extract_block(body) {
      // Parse the block text as markdown
      let arena = Arena::new(); // Temp arena for parsing fragment
      let root = parse_document(&arena, &content, &Options::default());

      for node in root.children() {
        // Find items or paragraphs
        let (target, content_node) = match &node.data.borrow().value {
          NodeValue::Item(_) => (node, node.first_child().unwrap_or(node)),
          NodeValue::Paragraph => (node, node),
          _ => continue,
        };

        let text = collect_text(content_node);
        if let Some((kind, _desc)) = parse_conventional_head(&text) {
          // We recreate the node in our main arena
          // We want the whole list item content, but stripped of the conventional prefix
          // potentially? Actually the standard is to keep it or format it.
          // Let's keep it simple: Clone the node logic.

          let item = ctx.new_node(NodeValue::Item(NodeList {
            list_type: comrak::nodes::ListType::Bullet,
            bullet_char: b'-',
            delimiter: comrak::nodes::ListDelimType::Period,
            tight: true,
            ..NodeList::default()
          }));

          // If it was a list item, copy all children. If paragraph, wrap in paragraph.
          if matches!(target.data.borrow().value, NodeValue::Item(_)) {
            for child in target.children() {
              item.append(ctx.deep_clone(child));
            }
          } else {
            let p = ctx.new_node(NodeValue::Paragraph);
            p.append(ctx.deep_clone(target));
            item.append(p);
          }

          inject_pr_meta(ctx, item, pr);
          entries.push((kind, item));
        }
      }
      return entries;
    }
  }

  // 2. Fallback to title
  if let Some((kind, desc)) = parse_conventional_head(&pr.title) {
    let text = format!("{} (#{} @{})", desc, pr.number, pr.author.login);
    entries.push((kind, ctx.new_list_item(&text)));
  } else {
    let text = format!("{} (#{} @{})", pr.title, pr.number, pr.author.login);
    entries.push((SectionKind::Internal, ctx.new_list_item(&text)));
  }

  entries
}

fn inject_pr_meta<'a>(ctx: &Context<'a>, item: &'a AstNode<'a>, pr: &PR) {
  let suffix = format!(" (#{} @{})", pr.number, pr.author.login);
  // Append to last text node of first paragraph, or create new
  if let Some(p) = item
    .children()
    .find(|n| matches!(n.data.borrow().value, NodeValue::Paragraph))
  {
    p.append(ctx.new_text(suffix));
  }
}

fn extract_block(text: &str) -> Option<String> {
  let s = text.find(MARKER_START)? + MARKER_START.len();
  let e = text.find(MARKER_END)?;
  if s < e { Some(text[s..e].trim().to_string()) } else { None }
}

fn parse_conventional_head(text: &str) -> Option<(SectionKind, &str)> {
  let (head, desc) = text.split_once(':')?;
  let type_scope = head
    .split_once('(')
    .map(|(t, _)| t)
    .unwrap_or(head);
  let kind = SectionKind::from_str(type_scope)?;
  Some((kind, desc.trim()))
}

fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
  let mut s = String::new();
  for c in node.children() {
    match &c.data.borrow().value {
      NodeValue::Text(t) | NodeValue::Code(comrak::nodes::NodeCode { literal: t, .. }) => {
        s.push_str(t)
      }
      _ => s.push_str(&collect_text(c)),
    }
  }
  s
}

// ============================================================================
// Utils
// ============================================================================

mod git {
  use super::*;
  pub fn get_merged_prs_since(ver: &Version) -> Result<Vec<PR>, Box<dyn Error>> {
    let date = get_tag_date(ver).ok_or(format!("Tag for {} not found", ver))?;
    let out = Command::new("gh")
      .args([
        "pr",
        "list",
        "--state",
        "merged",
        "--base",
        "master",
        "--limit",
        "500",
        "--json",
        "number,title,body,author,mergedAt",
      ])
      .output()?;
    if !out.status.success() {
      return Err(format!("gh failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }

    let prs: Vec<PR> = serde_json::from_slice(&out.stdout)?;
    Ok(
      prs
        .into_iter()
        .filter(|p| p.merged_at.as_ref().is_some_and(|d| d > &date))
        .collect(),
    )
  }

  fn get_tag_date(ver: &Version) -> Option<String> {
    let tags = [format!("v{}", ver), format!("ribir-v{}", ver), ver.to_string()];
    for tag in tags {
      if let Ok(o) = Command::new("git")
        .args(["log", "-1", "--format=%aI", &tag])
        .output()
      {
        if o.status.success() {
          return Some(
            String::from_utf8_lossy(&o.stdout)
              .trim()
              .to_string(),
          );
        }
      }
    }
    None
  }
}

fn today() -> String {
  String::from_utf8_lossy(
    &Command::new("date")
      .arg("+%Y-%m-%d")
      .output()
      .unwrap()
      .stdout,
  )
  .trim()
  .to_string()
}

fn get_arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
  args
    .iter()
    .position(|a| a == key)
    .and_then(|i| args.get(i + 1).map(|s| s.as_str()))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_section_parsing() {
    assert_eq!(SectionKind::from_str("feat"), Some(SectionKind::Features));
    assert_eq!(SectionKind::from_str("fix"), Some(SectionKind::Fixed));
    assert_eq!(SectionKind::from_str("unknown"), None);
  }

  #[test]
  fn test_conventional_head() {
    let (k, d) = parse_conventional_head("feat(ui): Add items").unwrap();
    assert_eq!(k, SectionKind::Features);
    assert_eq!(d, "Add items");
  }

  #[test]
  fn test_extract_block() {
    let msg = format!("foo\n{}\n- feat: bar\n{}\nbaz", MARKER_START, MARKER_END);
    assert_eq!(extract_block(&msg).unwrap(), "- feat: bar");
  }
}
