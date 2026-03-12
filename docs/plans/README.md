# Plans

Feature design documents and implementation plans used to build terminal-vibes.

## How It Works

Each feature goes through a two-document workflow:

1. **Design doc** (`YYYY-MM-DD-feature-name-design.md`) — explores the problem space, defines data structures, and lays out architectural decisions before any code is written.
2. **Implementation plan** (`YYYY-MM-DD-feature-name.md`) — a step-by-step build guide with exact file paths, code snippets, and verification commands. Designed to be executed task-by-task with TDD checkpoints.

Not every feature requires both documents. Small or straightforward changes may only need an implementation plan.

## Document Structure

### Design Docs

```
# Feature Name Design

## Goal
What problem this solves and why.

## Data Interface
Key types, structs, and data flow.

## [Feature-specific sections]
Architecture, algorithms, trade-offs, etc.
```

### Implementation Plans

```
# Feature Name Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans ...

**Goal:** One-sentence summary.
**Architecture:** How it fits into the codebase.
**Tech Stack:** Dependencies and tools.
**Design doc:** Link to companion design document (if one exists).

---

### Task N: Short task name

**Files:**
- Create: `src/path/to/new_file.rs`
- Modify: `src/path/to/existing_file.rs:line-range` (what changes)

**Step 1: Do the thing**
Code, commands, and expected output.

**Step 2: Verify**
Run: `cargo test ...`
Expected: All tests pass.

**Step 3: Commit**
```

## How Plans Are Built

1. **Brainstorm** — explore requirements, constraints, and existing patterns in the codebase.
2. **Design** — write the design doc covering data structures, architecture, and trade-offs.
3. **Plan** — break the implementation into small, sequential tasks. Each task should compile and pass tests independently. Include exact file paths, code snippets, and verification commands so the plan can be executed mechanically.
4. **Execute** — work through tasks one at a time. Commit after each task.
5. **Archive** — plans stay in this directory as a record of how and why things were built.

## Conventions

- **Date prefix** — `YYYY-MM-DD` for chronological sorting.
- **Kebab-case names** — `game-of-life`, `beat-detection`, `fullscreen-optimizations`.
- **Suffix `-design`** for design docs; no suffix for implementation plans.
- **Tasks are incremental** — each task produces a compiling, tested checkpoint.
- **Verification steps** — every task ends with a test/build command and expected output.
- **File annotations** — tasks list which files are created or modified (with line numbers when relevant).
