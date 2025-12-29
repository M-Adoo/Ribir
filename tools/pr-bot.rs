#!/usr/bin/env -S cargo +nightly -Zscript
---
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
pico-args = "0.5"
---

use std::{
  error::Error,
  io::Write,
  process::{Command, Output, Stdio},
};

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PRView {
  title: String,
  body: String,
}

#[derive(Deserialize)]
struct PRCommits {
  commits: Vec<Commit>,
}

#[derive(Deserialize)]
struct Commit {
  #[serde(rename = "messageHeadline")]
  message_headline: String,
  #[serde(rename = "messageBody")]
  message_body: String,
}

#[derive(Deserialize, Serialize)]
struct GeminiResponse {
  summary: String,
  changelog: String,
  #[serde(default)]
  skip_changelog: bool,
}

struct Config {
  pr_id: Option<String>,
  dry_run: bool,
  mode: Mode,
}

enum Mode {
  Auto,
  RegenerateAll(Option<String>),
  SummaryOnly(Option<String>),
  ChangelogOnly(Option<String>),
}

impl Mode {
  fn needs(&self, body: &str) -> (bool, bool) {
    match self {
      Self::Auto => (body.contains(SUMMARY_PLACEHOLDER), body.contains(CHANGELOG_PLACEHOLDER)),
      Self::RegenerateAll(_) => (true, true),
      Self::SummaryOnly(_) => (true, false),
      Self::ChangelogOnly(_) => (false, true),
    }
  }

  fn context(&self) -> Option<&str> {
    match self {
      Self::RegenerateAll(ctx) | Self::SummaryOnly(ctx) | Self::ChangelogOnly(ctx) => {
        ctx.as_deref()
      }
      Self::Auto => None,
    }
  }

  fn log_status(&self) {
    match self {
      Self::RegenerateAll(Some(ctx)) => eprintln!("⚡ Regenerating all with context: {ctx}"),
      Self::RegenerateAll(None) => eprintln!("⚡ Regenerating all content"),
      Self::SummaryOnly(Some(ctx)) => eprintln!("📝 Regenerating summary with context: {ctx}"),
      Self::SummaryOnly(None) => eprintln!("📝 Regenerating summary only"),
      Self::ChangelogOnly(Some(ctx)) => eprintln!("📋 Regenerating changelog with context: {ctx}"),
      Self::ChangelogOnly(None) => eprintln!("📋 Regenerating changelog only"),
      Self::Auto => {}
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const SUMMARY_PLACEHOLDER: &str =
  "> 🤖 *Leave this placeholder to let AI generate, or replace with your summary.*";
const CHANGELOG_PLACEHOLDER: &str =
  "> 🤖 *Leave this placeholder to let AI generate, or replace with your entries:*";
const CHANGELOG_START: &str = "<!-- RIBIR_CHANGELOG_START -->";
const CHANGELOG_END: &str = "<!-- RIBIR_CHANGELOG_END -->";
const SKIP_CHANGELOG_CHECKED: &str =
  "- [x] 🛠️ No changelog needed (tests, CI, infra, or unreleased fix)";

const PREFERRED_MODELS: &[&str] = &[
  "gemini-3-flash-preview",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite",
  "gemini-3-pro-preview",
  "gemini-2.5-pro",
];

const PROMPT_TEMPLATE: &str = r#"You are a helpful assistant that summarizes GitHub Pull Requests.

PR Title: {title}
PR Description:
{body}

Commits:
{commits}

TASK:
1. Generate a summary with this EXACT structure:
   **Context**: A short sentence explaining why this change is needed.
   **Changes**:
   - Bullet points describing what was changed.
   Use short, clear sentences. Avoid long, complex ones.
2. Determine if this PR should SKIP changelog:
   - Set skip_changelog=true for: CI/CD, bot updates, tests, internal tools, infrastructure.
   - Set skip_changelog=false for: features, bug fixes, breaking changes, docs, user-facing items.
3. If skip_changelog=false, generate changelog entries: `- type(scope): description`
   Types: feat, fix, change, docs, breaking
   Scopes: core, gpu, macros, widgets, themes, painter, cli, text, tools

OUTPUT: Return ONLY JSON with keys 'summary', 'changelog', and 'skip_changelog'.
Examples:
{{"summary": "**Context**: The renderer was slow on large trees.\n**Changes**:\n- Refactored rendering pipeline to use batching.\n- Improved performance by 40%.", "changelog": "- fix(core): prevent crash", "skip_changelog": false}}
{{"summary": "**Context**: CI failing on Windows.\n**Changes**:\n- Fixed path handling in workflow file.", "changelog": "", "skip_changelog": true}}"#;

const HELP: &str = r#"PR Bot - Generate PR summaries and changelog entries using Gemini AI.

USAGE:
    cargo +nightly -Zscript tools/pr-bot.rs [OPTIONS] [PR_ID]

OPTIONS:
    -h, --help                      Print this help message
    --dry-run                       Preview without updating PR
    --regenerate [CONTENT]          Regenerate summary and changelog
    --summary-only [CONTENT]        Regenerate only summary
    --changelog-only [CONTENT]      Regenerate only changelog

PR_ID:
    Optional PR number or URL. If omitted, uses current branch's PR.

EXAMPLES:
    # Regenerate with context
    cargo +nightly -Zscript tools/pr-bot.rs --regenerate "Adds OAuth2 support"
    
    # Regenerate summary only
    cargo +nightly -Zscript tools/pr-bot.rs --summary-only "Major core refactor"

REQUIREMENTS:
    - Rust nightly toolchain (for -Zscript)
    - gh CLI (authenticated)
    - GEMINI_API_KEY environment variable
"#;

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
  if let Err(e) = run() {
    eprintln!("Error: {e}");
    std::process::exit(1);
  }
}

fn run() -> Result<()> {
  let config = parse_args()?;

  if config.dry_run {
    eprintln!("🔍 Dry-run mode enabled");
  }
  config.mode.log_status();

  let pr = gh_json::<PRView>(config.pr_id.as_deref(), "title,body")?;
  let (needs_summary, needs_changelog) = config.mode.needs(&pr.body);

  if !needs_summary && !needs_changelog {
    println!("No placeholders found - skipping. Use --regenerate to force.");
    return Ok(());
  }

  let commits = gh_json::<PRCommits>(config.pr_id.as_deref(), "commits")?.commits;
  let commits_text = format_commits(&commits);

  let prompt = build_prompt(&pr, &commits_text, config.mode.context());
  let response = generate_content(&prompt)?;
  let updated_body = update_pr_body(&pr.body, &response, needs_summary, needs_changelog);

  if config.dry_run {
    print_preview(&updated_body);
  } else {
    gh_edit_body(config.pr_id.as_deref(), &updated_body)?;
    println!("✅ PR updated successfully!");
  }

  Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Argument Parsing
// ─────────────────────────────────────────────────────────────────────────────

fn parse_args() -> Result<Config> {
  let mut args = pico_args::Arguments::from_env();

  if args.contains(["-h", "--help"]) {
    print!("{HELP}");
    std::process::exit(0);
  }

  let dry_run = args.contains("--dry-run");

  // Parse mode flags with optional context value
  // Use opt_value_from_fn to handle both `--flag` and `--flag value` cases
  let mode = if let Some(ctx) = args.opt_value_from_str::<_, String>("--regenerate")? {
    Mode::RegenerateAll(Some(ctx))
  } else if args.contains("--regenerate") {
    Mode::RegenerateAll(None)
  } else if let Some(ctx) = args.opt_value_from_str::<_, String>("--summary-only")? {
    Mode::SummaryOnly(Some(ctx))
  } else if args.contains("--summary-only") {
    Mode::SummaryOnly(None)
  } else if let Some(ctx) = args.opt_value_from_str::<_, String>("--changelog-only")? {
    Mode::ChangelogOnly(Some(ctx))
  } else if args.contains("--changelog-only") {
    Mode::ChangelogOnly(None)
  } else {
    Mode::Auto
  };

  let pr_id = args.opt_free_from_str()?;

  let remaining = args.finish();
  if !remaining.is_empty() {
    return Err(format!("Unexpected arguments: {:?}", remaining).into());
  }

  Ok(Config { pr_id, dry_run, mode })
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Logic
// ─────────────────────────────────────────────────────────────────────────────

fn format_commits(commits: &[Commit]) -> String {
  if commits.is_empty() {
    "(No commits found)".into()
  } else {
    commits
      .iter()
      .map(|c| {
        if c.message_body.is_empty() {
          format!("- {}", c.message_headline)
        } else {
          format!("- {}\n  {}", c.message_headline, c.message_body.replace('\n', "\n  "))
        }
      })
      .collect::<Vec<_>>()
      .join("\n")
  }
}

fn build_prompt(pr: &PRView, commits: &str, context: Option<&str>) -> String {
  let base = PROMPT_TEMPLATE
    .replace("{title}", &pr.title)
    .replace("{body}", &pr.body)
    .replace("{commits}", commits);

  match context {
    Some(ctx) => format!("ADDITIONAL CONTEXT FROM USER:\n{}\n\n{}", ctx, base),
    None => base,
  }
}

fn generate_content(prompt: &str) -> Result<GeminiResponse> {
  let result = call_gemini_with_fallback(prompt)?;
  let json_str = extract_json(&result).ok_or("No JSON found in response")?;
  let response: GeminiResponse =
    serde_json::from_str(&json_str).map_err(|e| format!("Invalid JSON: {e}\nRaw: {result}"))?;
  sanitize_response(response)
}

fn update_pr_body(
  body: &str, response: &GeminiResponse, needs_summary: bool, needs_changelog: bool,
) -> String {
  let mut result = body.to_string();

  if needs_summary {
    result = result.replace(SUMMARY_PLACEHOLDER, &response.summary);
  }

  if needs_changelog {
    result = replace_changelog_section(&result, &response.changelog, response.skip_changelog);
  }

  result
}

fn replace_changelog_section(body: &str, changelog: &str, skip_changelog: bool) -> String {
  let result = body.to_string();

  // Determine content to insert
  let content =
    if skip_changelog { SKIP_CHANGELOG_CHECKED.to_string() } else { changelog.to_string() };

  // Try to find markers first for a clean replacement
  if let (Some(start_pos), Some(end_pos)) =
    (result.find(CHANGELOG_START), result.find(CHANGELOG_END))
  {
    let content_start = start_pos + CHANGELOG_START.len();
    if content_start < end_pos {
      return format!("{}\n\n{}\n\n{}", &result[..content_start], content, &result[end_pos..]);
    }
  }

  // Fallback: replace placeholder only
  let Some(start) = result.find(CHANGELOG_PLACEHOLDER) else {
    return result;
  };

  let after_placeholder = start + CHANGELOG_PLACEHOLDER.len();
  let end = find_code_block_end(&result, after_placeholder).unwrap_or(after_placeholder);

  format!("{}{}{}", &result[..start], content, &result[end..])
}

fn find_code_block_end(text: &str, start: usize) -> Option<usize> {
  let remaining = &text[start..];
  let block_start = remaining.find("```")?;
  let abs_start = start + block_start + 3;
  let block_end = text[abs_start..].find("```")?;
  Some(abs_start + block_end + 3)
}

fn print_preview(body: &str) {
  println!("\n📝 Preview:\n{}\n", "─".repeat(50));
  println!("{body}");
  println!("{}\n💡 Run without --dry-run to apply.", "─".repeat(50));
}

// ─────────────────────────────────────────────────────────────────────────────
// GitHub CLI
// ─────────────────────────────────────────────────────────────────────────────

fn gh_json<T: for<'de> Deserialize<'de>>(pr_id: Option<&str>, fields: &str) -> Result<T> {
  let mut args = vec!["pr", "view"];
  if let Some(id) = pr_id {
    args.push(id);
  }
  args.extend(["--json", fields]);

  let output = run_command("gh", &args)?;
  Ok(serde_json::from_slice(&output.stdout)?)
}

fn gh_edit_body(pr_id: Option<&str>, body: &str) -> Result<()> {
  let mut args = vec!["pr", "edit"];
  if let Some(id) = pr_id {
    args.push(id);
  }
  args.extend(["--body", body]);

  run_command("gh", &args)?;
  Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<Output> {
  let output = Command::new(cmd).args(args).output()?;
  if !output.status.success() {
    return Err(
      format!("{} {:?} failed: {}", cmd, args, String::from_utf8_lossy(&output.stderr)).into(),
    );
  }
  Ok(output)
}

// ─────────────────────────────────────────────────────────────────────────────
// Gemini API
// ─────────────────────────────────────────────────────────────────────────────

fn call_gemini_with_fallback(prompt: &str) -> Result<String> {
  let mut last_error = String::new();

  for model in PREFERRED_MODELS {
    eprintln!("Trying model: {model}");
    match call_gemini(prompt, model) {
      Ok(res) => {
        eprintln!("✓ Success: {model}");
        return Ok(res);
      }
      Err(e) => {
        eprintln!("✗ Failed: {model} - {e}");
        last_error = e;
      }
    }
  }

  Err(format!("All models failed. Last error: {last_error}").into())
}

fn call_gemini(prompt: &str, model: &str) -> std::result::Result<String, String> {
  let mut child = Command::new("gemini")
    .args(["--model", model, "--approval-mode", "yolo", "-o", "text"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| e.to_string())?;

  if let Some(mut stdin) = child.stdin.take() {
    stdin
      .write_all(prompt.as_bytes())
      .map_err(|e| e.to_string())?;
  }

  let output = child
    .wait_with_output()
    .map_err(|e| e.to_string())?;

  if output.status.success() {
    Ok(String::from_utf8_lossy(&output.stdout).into())
  } else {
    Err(String::from_utf8_lossy(&output.stderr).into())
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Response Processing
// ─────────────────────────────────────────────────────────────────────────────

fn extract_json(s: &str) -> Option<String> {
  let start = s.find('{')?;
  let end = s.rfind('}')?;
  Some(s[start..=end].to_string())
}

fn sanitize_response(mut response: GeminiResponse) -> Result<GeminiResponse> {
  response.summary = sanitize_markdown(&response.summary);
  response.changelog = sanitize_markdown(&response.changelog);

  if response.summary.trim().is_empty() {
    return Err("Empty summary".into());
  }

  // Only validate changelog format if not skipping
  if !response.skip_changelog
    && !response
      .changelog
      .lines()
      .any(|l| l.trim().starts_with('-'))
  {
    return Err("Invalid changelog format".into());
  }

  truncate(&mut response.summary, 1000, "...");
  truncate(&mut response.changelog, 5000, "\n...(truncated)");

  Ok(response)
}

fn sanitize_markdown(s: &str) -> String {
  s.lines()
    .filter(|line| {
      let lower = line.to_lowercase();
      !lower.contains("<script") && !lower.contains("<iframe") && !lower.contains("javascript:")
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn truncate(s: &mut String, max_len: usize, suffix: &str) {
  if s.len() > max_len {
    *s = s.chars().take(max_len).collect();
    s.push_str(suffix);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_update_pr_body() {
    let body = "## Summary\n> 🤖 *Leave this placeholder to let AI generate, or replace with your \
                summary.*\n\n## Changelog\n> 🤖 *Leave this placeholder to let AI generate, or \
                replace with your entries:*\n>\n> ```\n> - feat(widgets): add Tooltip\n> \
                ```\n\nOther content";
    let response =
      GeminiResponse { summary: "Fixed a bug.".into(), changelog: "- fix(core): fix crash".into() };

    let updated = update_pr_body(body, &response, true, true);
    assert!(updated.contains("Fixed a bug."));
    assert!(updated.contains("- fix(core): fix crash"));
    assert!(!updated.contains("Tooltip"));
    assert!(!updated.contains("placeholder"));
    assert!(updated.contains("Other content"));
  }

  #[test]
  fn test_sanitize_markdown() {
    let input = "Normal\n<script>alert('xss')</script>\nOK";
    let result = sanitize_markdown(input);
    assert!(!result.contains("<script"));
    assert!(result.contains("Normal"));
    assert!(result.contains("OK"));
  }

  #[test]
  fn test_sanitize_response_valid() {
    let response =
      GeminiResponse { summary: "New feature".into(), changelog: "- feat(core): add".into() };
    assert!(sanitize_response(response).is_ok());
  }

  #[test]
  fn test_sanitize_response_empty_summary() {
    let response = GeminiResponse { summary: "   ".into(), changelog: "- feat: x".into() };
    assert!(sanitize_response(response).is_err());
  }

  #[test]
  fn test_sanitize_response_invalid_changelog() {
    let response = GeminiResponse { summary: "OK".into(), changelog: "no bullets".into() };
    assert!(sanitize_response(response).is_err());
  }

  #[test]
  fn test_truncate() {
    let mut s = "hello world".to_string();
    truncate(&mut s, 5, "...");
    assert_eq!(s, "hello...");
  }

  #[test]
  fn test_mode_needs() {
    let body_with_both = format!("{}\n{}", SUMMARY_PLACEHOLDER, CHANGELOG_PLACEHOLDER);
    assert_eq!(Mode::Auto.needs(&body_with_both), (true, true));
    assert_eq!(Mode::Auto.needs("no placeholders"), (false, false));
    assert_eq!(Mode::RegenerateAll(None).needs(""), (true, true));
    assert_eq!(Mode::SummaryOnly(None).needs(""), (true, false));
    assert_eq!(Mode::ChangelogOnly(None).needs(""), (false, true));
  }
}
