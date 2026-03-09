# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.4](https://github.com/joelmgallant/terminal-vibes/compare/v1.3.3...v1.3.4) - 2026-03-09

### Other

- improve mermaid diagrams with color fills and clearer layout

## [1.3.3](https://github.com/joelmgallant/terminal-vibes/compare/v1.3.2...v1.3.3) - 2026-03-09

### Other

- add mermaid diagrams to color detail performance section

## [1.3.2](https://github.com/joelmgallant/terminal-vibes/compare/v1.3.1...v1.3.2) - 2026-03-09

### Other

- add contribution guide and color detail performance explanation
- update CLAUDE.md with fullscreen optimization details
- add release process section to CLAUDE.md

## [1.3.1](https://github.com/joelmgallant/terminal-vibes/compare/v1.3.0...v1.3.1) - 2026-03-09

### Fixed

- raise radial brightness floor from 30% to 60%

## [1.3.0](https://github.com/joelmgallant/terminal-vibes/compare/v1.2.0...v1.3.0) - 2026-03-09

### Added

- add color_detail control with adaptive quantization
- add adaptive quantization step based on terminal size and color detail

### Fixed

- prevent frame budget oscillation and step=0 division
- remap color_detail keys from d/D to [/] to avoid starfield conflict
- restore SinLut removed by Task 2 and add step=0 guard

### Other

- add frame time budget monitoring with auto color detail adjustment
- default to 60fps when not running in tmux
- quantize colors at canvas set-time for better ratatui diffing
- parameterize quantize_color step size
- add fullscreen optimizations implementation plan
- add fullscreen rendering optimizations design

## [1.2.0](https://github.com/joelmgallant/terminal-vibes/compare/v1.1.0...v1.2.0) - 2026-03-08

### Added

- *(ui)* use predicted beats for tighter BEAT! indicator
- *(ui)* show BPM in status bar with confidence indicator
- *(beat)* wire TempoEstimator into BeatDetector pipeline
- *(beat)* implement onset autocorrelation BPM estimation
- *(beat)* add TempoEstimator skeleton with onset strength buffer
- *(beat)* add tempo estimation config fields
- *(beat)* add TempoData struct and wire into FrameData

### Fixed

- *(ci)* fix YAML quoting for release-plz path
- *(ci)* force reinstall release-plz to avoid stale cache
- *(ci)* use full path for release-plz binary
- *(ci)* add cargo bin to PATH for release-plz CLI
- *(ci)* use release-plz update + release for fully automatic publishing
- *(beat)* measure actual processing FPS instead of assuming 60 Hz
- resolve all clippy warnings for CI compatibility
- *(ci)* correct release-plz action reference
- suppress dead_code warnings on per-band beat fields

### Other

- add pre-commit hook for fmt, clippy, and tests
- *(beat)* suppress expected dead_code warnings on TempoData fields
- fix formatting in beat.rs
- *(beat)* add tempo change convergence test
- *(beat)* add phase tracking and prediction tests
- fix formatting in beat.rs and ui.rs
- add auto-release pipeline design and implementation plan
- add automatic release workflow with release-plz
- add release-plz configuration
- add check workflow for fmt, clippy, and tests
- add BPM estimation implementation plan
- add BPM estimation and beat prediction design
