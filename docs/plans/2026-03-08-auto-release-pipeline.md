# Auto-Release Pipeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fully automatic version bumping, crates.io publishing, and GitHub Releases triggered by conventional commits pushed to `trunk`.

**Architecture:** Two GitHub Actions workflows — one for CI checks (fmt/clippy/test) and one for release via `release-plz`. release-plz parses conventional commits, bumps `Cargo.toml` version, generates `CHANGELOG.md`, creates git tags, publishes to crates.io, and creates GitHub Releases.

**Tech Stack:** GitHub Actions, release-plz (v0.5 action), cargo, git-cliff (via release-plz)

---

### Prerequisites (Manual — User Must Do)

1. Go to https://crates.io/settings/tokens and create an API token scoped to `terminal-vibes`
2. Go to GitHub repo → Settings → Secrets → Actions → New repository secret
3. Add secret named `CARGO_REGISTRY_TOKEN` with the token value

---

### Task 1: Create CI Workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Create the workflow directory**

```bash
mkdir -p .github/workflows
```

**Step 2: Write the CI workflow file**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [trunk]
  pull_request:
    branches: [trunk]

jobs:
  check:
    name: Check
    runs-on: macos-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry & build
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Tests
        run: cargo test --lib
```

**Step 3: Validate the YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: No output (valid YAML)

**Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add check workflow for fmt, clippy, and tests"
```

---

### Task 2: Create release-plz Configuration

**Files:**
- Create: `release-plz.toml`

**Step 1: Write the release-plz config**

Create `release-plz.toml`:

```toml
[workspace]
# Create/update CHANGELOG.md from conventional commits
changelog_update = true

# Create git tags like v1.2.0
git_tag_enable = true

# Create GitHub Releases with changelog body
git_release_enable = true

# Release on every push, not just when merging a release PR
release_always = true

# Single-crate repo, use simple tag format
git_tag_name = "v{{ version }}"
git_release_name = "v{{ version }}"
```

**Step 2: Commit**

```bash
git add release-plz.toml
git commit -m "chore: add release-plz configuration"
```

---

### Task 3: Create Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Write the release workflow file**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    branches: [trunk]

permissions:
  contents: write

jobs:
  release:
    name: Release
    runs-on: macos-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry & build
        uses: Swatinem/rust-cache@v2

      - name: Run release-plz
        uses: release-plz/release-plz-action@v0.5
        with:
          command: release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

**Step 2: Validate the YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: No output (valid YAML)

**Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add automatic release workflow with release-plz"
```

---

### Task 4: Create Baseline Tag

release-plz needs an existing tag to diff against. Current version is 1.1.0.

**Step 1: Create the tag on current HEAD**

```bash
git tag v1.1.0
```

**Step 2: Verify the tag**

Run: `git tag -l`
Expected: `v1.1.0`

---

### Task 5: Push Everything

**Step 1: Push commits and tag to remote**

```bash
git push origin trunk
git push origin v1.1.0
```

**Step 2: Verify workflows appear in GitHub Actions**

Run: `gh run list --limit 5`
Expected: CI and Release workflows should appear as running or queued.

---

### Post-Implementation Verification

1. Check GitHub Actions tab — CI workflow should run (fmt/clippy/test)
2. Release workflow should run but create no release (commits so far are `ci:` and `chore:` prefixes)
3. To test a real release, push a commit like `fix: test release pipeline` — should bump to 1.1.1 and publish

### Rollback

If something goes wrong:
- Delete the GitHub Release from the releases page
- `git tag -d v1.x.x && git push origin :refs/tags/v1.x.x` to remove a bad tag
- `cargo yank --version 1.x.x` to yank a bad crates.io publish (doesn't delete, just marks unusable)
