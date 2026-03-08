# Auto-Version Increment & Release Pipeline Design

## Goal

Fully automatic release pipeline: push conventional commits to `trunk` → CI checks → version bump → crates.io publish → GitHub Release with changelog.

## Architecture

### Two Workflows

**1. CI Checks (`.github/workflows/ci.yml`)**
- Triggers: push to `trunk`, PRs targeting `trunk`
- Runner: `macos-latest` (Core Audio deps require macOS)
- Steps: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`

**2. Release (`.github/workflows/release.yml`)**
- Triggers: push to `trunk` only
- Runner: `macos-latest`
- Tool: `release-plz` GitHub Action
- Steps: runs `release-plz release` which handles version bump, changelog, tag, crates.io publish, GitHub Release

### Version Bump Rules (Conventional Commits)

| Commit prefix | Bump | Example |
|---|---|---|
| `fix:` | patch | 1.1.0 → 1.1.1 |
| `feat:` | minor | 1.1.0 → 1.2.0 |
| `feat!:` / `BREAKING CHANGE:` | major | 1.1.0 → 2.0.0 |
| `chore:`, `docs:`, `style:` | none | no release |

### Configuration

**`release-plz.toml`** (repo root):
- Enable changelog updates, git tags, and GitHub Releases

**Secrets** (GitHub repo settings):
- `CARGO_REGISTRY_TOKEN` — crates.io API token

### One-Time Setup

1. Create `v1.1.0` tag on current HEAD as baseline for release-plz
2. Add `CARGO_REGISTRY_TOKEN` secret in GitHub repo settings

## Files to Create

1. `.github/workflows/ci.yml`
2. `.github/workflows/release.yml`
3. `release-plz.toml`

## Flow

```
push to trunk
  → ci.yml: fmt check, clippy, tests
  → release.yml: release-plz release
    → parse conventional commits since last tag
    → determine bump level
    → update Cargo.toml version
    → update CHANGELOG.md
    → commit version bump + changelog
    → create git tag (vX.Y.Z)
    → cargo publish
    → create GitHub Release with changelog
```
