# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.7.0](https://github.com/joelmgallant/terminal-vibes/compare/v1.6.6...v1.7.0) - 2026-03-12

### Added

- *(gui)* add [gui] config section, status in window title
- *(gui)* shader hot-reload with built-in presets
- *(gui)* integrate audio pipeline into GPU event loop
- *(gui)* fullscreen shader pipeline with spectrum_rings shader
- *(gui)* add audio texture and uniform buffer conversion
- *(gui)* add wgpu window with basic render surface

### Other

- add GUI mode build commands, fix clippy warnings
- reorganize app into terminal/ submodule, add --gui flag
- extract AudioPipeline for shared audio+processing setup
- wire app crate with core re-exports, migrate tests
- scaffold cargo workspace with core and app crates
