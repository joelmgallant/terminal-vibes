# Game of Life Visualization Design

## Overview

A Conway's Game of Life visualization that deeply fuses audio reactivity with cellular automata. Audio both seeds new cells and warps the simulation rules, creating an organic, music-driven emergent system. Three interchangeable render modes offer different visual aesthetics from the same underlying simulation.

## Core Simulation

Double-buffered flat grid using two `Vec<Cell>` buffers (`current` and `next`), swapped each tick.

```rust
struct Cell {
    alive: bool,
    age: u16, // frames alive, caps at ~1000
}
```

- Grid dimensions match the active render mode's pixel resolution
- Toroidal wrapping (edges connect) via modular arithmetic on neighbor lookups
- Tick rate: ~30 generations/sec (every 2 frames at 60fps)
- Beat hits trigger an extra tick for "time acceleration" effect

## Audio-Simulation Interaction

### Seeding (audio spawns cells)

- Frequency bands map spatially: bass seeds bottom third, mid seeds middle, treble seeds top third
- On beat detection (`frame.beat.beat`), spawn a classic pattern (glider, blinker, r-pentomino) at a random position; heavier beats spawn larger patterns
- RMS scales overall spawn density: quiet passages let the simulation breathe, loud sections flood it

### Rule Warping (audio bends B3/S23)

- Base rules: Birth on 3 neighbors, Survive on 2-3 (classic Conway)
- Bass envelope shifts birth threshold: high bass allows birth on 2 neighbors (more growth)
- Treble envelope shifts survival: high treble allows survival on 1-4 (more resilient structures)
- Result: drops = explosive growth, quiet sections = natural die-off

## Render Modes

Three modes within a single visualization, toggled with `m` key. All share the same simulation logic. Grid reinitializes when switching modes (resolution changes).

| Mode | Resolution | Rendering | `heavy_rendering` |
|------|-----------|-----------|-------------------|
| Character (default) | cols x rows | `█▓▒░` by age, direct buffer | `false` |
| HalfBlock | cols x (rows x 2) | `HalfBlockCanvas`, full RGB | `true` |
| Braille | (cols x 2) x (rows x 4) | `BrailleCanvas`, dot patterns | `false` |

## Coloring

- Cell age normalized to 0.0-1.0 (capped at ~200 frames)
- Mapped through active `ColorPalette` — newborn at gradient start, ancient at gradient end
- Beat envelope multiplies brightness: `0.3 + 0.7 * envelope`
- Dead cells are empty (transparent)
- `quantize_color()` applied with adaptive `quant_step`
- Palette cycling via `p`/`P` keys (reuses existing `ColorPalette` system)

## Key Bindings

| Key | Action |
|-----|--------|
| `m` | Cycle render mode (Character -> HalfBlock -> Braille) |
| `p`/`P` | Cycle palette forward/backward |
| `r` | Randomize grid (fresh start) |
| `c` | Clear grid (kill all cells) |

## Config Persistence

```toml
[visualizations.life]
render_mode = "character"
palette = "spectrum"
```

## Architecture

- Single file: `src/visualizations/life.rs`
- Registered as one entry in `VisualizationRegistry` with name `"life"`
- Module declared in `src/visualizations/mod.rs`
- Integration test in `tests/life_test.rs`
- Approach: double-buffered flat `Vec<Cell>`, cache-friendly, zero allocations after init

## Performance Considerations

- Grid is at most ~10K cells in character mode, ~80K in braille mode — trivial computation
- Reuse canvas buffers via `resize_or_clear()`
- Apply `quantize_color()` for escape sequence reduction in HalfBlock mode
- `heavy_rendering()` returns `true` only when in HalfBlock mode
