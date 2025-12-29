# Developer Documentation

This directory contains internal documentation for Ribir project maintainers and contributors.

## 📋 Documentation Index

| Document | Description |
|----------|-------------|
| [Changelog Management](changelog-management.md) | PR Bot and Changelog Bot workflow |

## 🔧 Tools Quick Reference

All tools are self-documented. Run with `--help` for usage:

```bash
# CI tool - mirrors GitHub Actions locally
cargo +nightly ci --help

# PR Bot - AI-powered PR summary and changelog generation
cargo +nightly -Zscript tools/pr-bot.rs --help

# Changelog Bot - Collect and manage changelog entries
cargo +nightly -Zscript tools/changelog-bot.rs --help
```

## 📚 Related Root-Level Docs

| Document | Description |
|----------|-------------|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contribution guidelines |
| [RELEASE.md](../RELEASE.md) | Release process and branch management |
| [ROADMAP.md](../ROADMAP.md) | Project roadmap |
