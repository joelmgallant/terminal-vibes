# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
