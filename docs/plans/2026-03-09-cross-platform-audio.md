# Cross-Platform Audio Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Windows (WASAPI loopback) and Linux (PulseAudio monitor) audio capture backends alongside the existing macOS Core Audio backend, using conditional compilation to swap modules at build time.

**Architecture:** Each platform gets its own audio module file (`tap.rs`, `wasapi.rs`, `pulse.rs`) exporting an `AudioTap` struct with identical public API. `audio/mod.rs` uses `#[cfg(target_os)]` to select which module compiles. Everything downstream of the ring buffer is untouched.

**Tech Stack:** `windows` crate (v0.58) for WASAPI FFI, `libpulse-binding` + `libpulse-simple-binding` (v2.28) for PulseAudio, existing `ringbuf` for the producer/consumer contract.

**Design doc:** `docs/plans/2026-03-09-cross-platform-audio-design.md`

---

### Task 1: Gate existing macOS module with cfg

**Files:**
- Modify: `src/audio/mod.rs` (all lines, 1-18)
- Modify: `src/main.rs:82-84` (error message)
- Modify: `Cargo.toml:5` (package description)

**Step 1: Update `src/audio/mod.rs` to cfg-gate the macOS tap module**

Replace the entire file contents with:

```rust
#[cfg(target_os = "macos")]
mod tap;
#[cfg(target_os = "macos")]
pub use tap::AudioTap;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: f32,
    pub channels: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            channels: 2,
        }
    }
}
```

**Step 2: Update the error message in `src/main.rs:81-84`**

Change the `.context(...)` on the `AudioTap::new()` call to be platform-aware:

```rust
    let _audio_tap = AudioTap::new(producer, audio_config.clone()).context(
        if cfg!(target_os = "macos") {
            "Failed to start audio capture. \
             Make sure you're on macOS 15+ and have granted audio permissions."
        } else if cfg!(target_os = "windows") {
            "Failed to start audio capture. \
             Make sure an audio output device is available."
        } else {
            "Failed to start audio capture. \
             Make sure PulseAudio or PipeWire is running."
        },
    )?;
```

**Step 3: Update package description in `Cargo.toml:5`**

Change:
```toml
description = "Terminal-based music visualizer for macOS system audio"
```
To:
```toml
description = "Terminal-based music visualizer for system audio"
```

**Step 4: Verify it still compiles and passes tests on macOS**

Run: `cargo build && cargo test --lib`
Expected: clean build, all tests pass (macOS cfg gate is active)

**Step 5: Commit**

```bash
git add src/audio/mod.rs src/main.rs Cargo.toml
git commit -m "refactor: cfg-gate macOS audio module for cross-platform support"
```

---

### Task 2: Add Windows WASAPI loopback backend

**Files:**
- Create: `src/audio/wasapi.rs`
- Modify: `src/audio/mod.rs` (add 3 lines)
- Modify: `Cargo.toml` (add Windows dependencies)

**Step 1: Add Windows dependencies to `Cargo.toml`**

Add after the `[target.'cfg(target_os = "macos")'.dependencies]` block (after line 28):

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_Devices_Properties",
] }
```

**Step 2: Add Windows module to `src/audio/mod.rs`**

Add after the macOS cfg lines:

```rust
#[cfg(target_os = "windows")]
mod wasapi;
#[cfg(target_os = "windows")]
pub use wasapi::AudioTap;
```

**Step 3: Create `src/audio/wasapi.rs`**

```rust
use super::AudioConfig;
use anyhow::{anyhow, Result};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::Interface;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

pub struct AudioTap {
    running: Arc<AtomicBool>,
    capture_thread: Option<thread::JoinHandle<()>>,
}

unsafe impl Send for AudioTap {}

impl AudioTap {
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let channels = config.channels;

        // Initialize COM and get audio client on the capture thread
        // (COM objects must be used on the thread where they were created)
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<()>>(1);

        let capture_thread = thread::spawn(move || {
            if let Err(e) = Self::capture_loop(producer, channels, &running_clone, &init_tx) {
                log::error!("WASAPI capture error: {}", e);
                let _ = init_tx.try_send(Err(e));
            }
        });

        // Wait for initialization result from capture thread
        let init_result = init_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow!("Audio capture initialization timed out"))?;
        init_result?;

        log::info!(
            "WASAPI loopback capture started (sample_rate={}, channels={})",
            config.sample_rate,
            config.channels
        );

        Ok(Self {
            running,
            capture_thread: Some(capture_thread),
        })
    }

    fn capture_loop(
        mut producer: HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
        init_tx: &std::sync::mpsc::SyncSender<Result<()>>,
    ) -> Result<()> {
        unsafe {
            // Initialize COM for this thread
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .map_err(|e| anyhow!("COM init failed: {}", e))?;

            let result = Self::capture_loop_inner(&mut producer, channels, running, init_tx);

            CoUninitialize();
            result
        }
    }

    unsafe fn capture_loop_inner(
        producer: &mut HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
        init_tx: &std::sync::mpsc::SyncSender<Result<()>>,
    ) -> Result<()> {
        // Get default audio output device
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| anyhow!("Failed to create device enumerator: {}", e))?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| anyhow!("No audio output device found: {}", e))?;

        // Get audio client
        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| anyhow!("Failed to activate audio client: {}", e))?;

        // Get the mix format (native device format)
        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| anyhow!("Failed to get mix format: {}", e))?;
        let mix_format = &*mix_format_ptr;

        let sample_rate = mix_format.nSamplesPerSec;
        let device_channels = mix_format.nChannels;

        log::debug!(
            "WASAPI device format: {}Hz, {} channels, {} bits",
            sample_rate,
            device_channels,
            mix_format.wBitsPerSample
        );

        // Initialize in loopback mode
        // AUDCLNT_STREAMFLAGS_LOOPBACK captures what's playing on the device
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                // Buffer duration: 100ms in 100-nanosecond units
                1_000_000,
                0,
                mix_format_ptr,
                None,
            )
            .map_err(|e| anyhow!("Failed to initialize loopback capture: {}", e))?;

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| anyhow!("Failed to get capture client: {}", e))?;

        // Start capturing
        audio_client
            .Start()
            .map_err(|e| anyhow!("Failed to start audio capture: {}", e))?;

        // Signal successful initialization
        let _ = init_tx.send(Ok(()));

        log::info!(
            "WASAPI loopback started: {}Hz, {} channels",
            sample_rate,
            device_channels
        );

        // Capture loop — poll every ~10ms
        while running.load(Ordering::Relaxed) {
            let packet_size = match capture_client.GetNextPacketSize() {
                Ok(size) => size,
                Err(e) => {
                    log::warn!("GetNextPacketSize failed: {}", e);
                    break;
                }
            };

            if packet_size == 0 {
                thread::sleep(Duration::from_millis(10));
                continue;
            }

            let mut buffer_ptr = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;

            if let Err(e) = capture_client.GetBuffer(
                &mut buffer_ptr,
                &mut num_frames,
                &mut flags,
                None,
                None,
            ) {
                log::warn!("GetBuffer failed: {}", e);
                break;
            }

            if num_frames > 0 && !buffer_ptr.is_null() {
                let is_silent = (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0;

                if is_silent {
                    // Push zeros for silent buffers
                    for _ in 0..num_frames {
                        let _ = producer.try_push(0.0);
                    }
                } else {
                    // Cast to f32 samples
                    let total_samples = (num_frames * device_channels as u32) as usize;
                    let samples =
                        std::slice::from_raw_parts(buffer_ptr as *const f32, total_samples);

                    // Stereo-to-mono downmix (same as macOS callback)
                    if channels >= 2 && device_channels >= 2 {
                        for chunk in samples.chunks(device_channels as usize) {
                            let mono =
                                chunk.iter().sum::<f32>() / device_channels as f32;
                            let _ = producer.try_push(mono);
                        }
                    } else {
                        for &sample in samples {
                            let _ = producer.try_push(sample);
                        }
                    }
                }
            }

            let _ = capture_client.ReleaseBuffer(num_frames);
        }

        // Stop capture
        let _ = audio_client.Stop();

        Ok(())
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }
        log::info!("WASAPI loopback capture stopped");
    }
}
```

**Step 4: Verify macOS build is unaffected**

Run: `cargo build && cargo test --lib`
Expected: clean build, all tests pass (Windows code is cfg-gated out)

**Step 5: Commit**

```bash
git add src/audio/wasapi.rs src/audio/mod.rs Cargo.toml
git commit -m "feat: add Windows WASAPI loopback audio capture backend"
```

---

### Task 3: Add Linux PulseAudio monitor source backend

**Files:**
- Create: `src/audio/pulse.rs`
- Modify: `src/audio/mod.rs` (add 3 lines)
- Modify: `Cargo.toml` (add Linux dependencies)

**Step 1: Add Linux dependencies to `Cargo.toml`**

Add after the Windows dependencies block:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libpulse-binding = "2.28"
libpulse-simple-binding = "2.28"
```

**Step 2: Add Linux module to `src/audio/mod.rs`**

Add after the Windows cfg lines:

```rust
#[cfg(target_os = "linux")]
mod pulse;
#[cfg(target_os = "linux")]
pub use pulse::AudioTap;
```

**Step 3: Create `src/audio/pulse.rs`**

```rust
use super::AudioConfig;
use anyhow::{anyhow, Result};
use libpulse_binding as pulse;
use libpulse_simple_binding::Simple;
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub struct AudioTap {
    running: Arc<AtomicBool>,
    capture_thread: Option<thread::JoinHandle<()>>,
}

unsafe impl Send for AudioTap {}

impl AudioTap {
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let channels = config.channels;
        let sample_rate = config.sample_rate as u32;

        // Test connection before spawning thread
        let spec = pulse::sample::Spec {
            format: pulse::sample::Format::F32le,
            channels: channels as u8,
            rate: sample_rate,
        };

        if !spec.is_valid() {
            return Err(anyhow!(
                "Invalid PulseAudio sample spec: {}Hz, {} channels",
                sample_rate,
                channels
            ));
        }

        // Connect to PulseAudio — use default sink's monitor source
        // @DEFAULT_SINK@.monitor works on both PulseAudio and PipeWire
        let simple = Simple::new(
            None,                             // Default server
            "terminal-vibes",                 // Application name
            pulse::stream::Direction::Record, // We're recording
            Some("@DEFAULT_SINK@.monitor"),   // Monitor source of default output
            "audio-capture",                  // Stream description
            &spec,
            None, // Default channel map
            None, // Default buffering attributes
        )
        .map_err(|e| {
            anyhow!(
                "Failed to connect to PulseAudio: {}. \
                 Is PulseAudio or PipeWire running?",
                e
            )
        })?;

        log::info!(
            "PulseAudio monitor capture connected (sample_rate={}, channels={})",
            sample_rate,
            channels
        );

        let capture_thread = thread::spawn(move || {
            Self::capture_loop(simple, producer, channels, &running_clone);
        });

        Ok(Self {
            running,
            capture_thread: Some(capture_thread),
        })
    }

    fn capture_loop(
        simple: Simple,
        mut producer: HeapProd<f32>,
        channels: u32,
        running: &AtomicBool,
    ) {
        // Read buffer: ~10ms of audio at 44100Hz stereo = ~1764 samples = ~3528 f32s
        let buf_frames = 1764;
        let buf_size = buf_frames * channels as usize;
        let mut buf = vec![0.0f32; buf_size];

        // Reinterpret f32 slice as u8 slice for PulseAudio read API
        let byte_len = buf_size * std::mem::size_of::<f32>();

        while running.load(Ordering::Relaxed) {
            let byte_slice = unsafe {
                std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, byte_len)
            };

            match simple.read(byte_slice) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("PulseAudio read error: {}", e);
                    break;
                }
            }

            // Stereo-to-mono downmix (same as macOS and Windows)
            if channels >= 2 {
                for chunk in buf.chunks(channels as usize) {
                    let mono = chunk.iter().sum::<f32>() / channels as f32;
                    let _ = producer.try_push(mono);
                }
            } else {
                for &sample in &buf {
                    let _ = producer.try_push(sample);
                }
            }
        }

        log::info!("PulseAudio capture loop ended");
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }
        log::info!("PulseAudio monitor capture stopped");
    }
}
```

**Step 4: Verify macOS build is unaffected**

Run: `cargo build && cargo test --lib`
Expected: clean build, all tests pass (Linux code is cfg-gated out)

**Step 5: Commit**

```bash
git add src/audio/pulse.rs src/audio/mod.rs Cargo.toml
git commit -m "feat: add Linux PulseAudio monitor source audio capture backend"
```

---

### Task 4: Add compile-time guard for unsupported platforms

**Files:**
- Modify: `src/audio/mod.rs` (add compile_error)

**Step 1: Add a compile-time error for unsupported platforms**

Add to the bottom of `src/audio/mod.rs`, after the cfg-gated module imports:

```rust
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
compile_error!(
    "terminal-vibes requires macOS, Windows, or Linux. \
     No audio capture backend is available for this platform."
);
```

**Step 2: Verify build**

Run: `cargo build && cargo test --lib`
Expected: clean build (macOS is a supported platform)

**Step 3: Commit**

```bash
git add src/audio/mod.rs
git commit -m "feat: add compile-time error for unsupported platforms"
```

---

### Task 5: Add cross-platform CI matrix

**Files:**
- Modify: `.github/workflows/ci.yml` (expand to matrix)

**Step 1: Update CI workflow to build on all three platforms**

Replace the entire `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [trunk]
  pull_request:
    branches: [trunk]

jobs:
  check:
    name: Check (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, windows-latest, ubuntu-latest]
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Install PulseAudio dev libraries (Linux)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libpulse-dev

      - name: Cache cargo registry & build
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Tests
        run: cargo test --lib
```

**Step 2: Verify the YAML is valid**

Run: `cat .github/workflows/ci.yml` and visually confirm structure.

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add cross-platform build matrix (macOS, Windows, Linux)"
```

---

### Task 6: Add mock audio boundary test

**Files:**
- Create: `tests/audio_boundary_test.rs`

**Step 1: Write test that validates the ring buffer contract**

This test pushes known samples into a ring buffer producer and verifies the processing pipeline can consume them — proving the audio → processing boundary works regardless of platform backend.

```rust
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

/// Validates the ring buffer contract that all audio backends must satisfy:
/// Producer pushes mono f32 samples, consumer drains them.
#[test]
fn ring_buffer_contract_mono_samples_flow_through() {
    let rb = HeapRb::<f32>::new(4096);
    let (mut producer, mut consumer) = rb.split();

    // Simulate audio callback pushing mono samples
    let test_signal: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
    for &sample in &test_signal {
        assert!(producer.try_push(sample).is_ok());
    }

    // Simulate processor draining
    let mut drain_buf = vec![0.0f32; 4096];
    let count = consumer.pop_slice(&mut drain_buf);
    assert_eq!(count, 1024);

    // Verify samples match
    for i in 0..1024 {
        assert!(
            (drain_buf[i] - test_signal[i]).abs() < f32::EPSILON,
            "Sample {} mismatch: {} vs {}",
            i,
            drain_buf[i],
            test_signal[i]
        );
    }
}

/// Validates graceful overflow: when buffer is full, try_push drops samples without panic.
#[test]
fn ring_buffer_contract_overflow_drops_gracefully() {
    let rb = HeapRb::<f32>::new(64);
    let (mut producer, _consumer) = rb.split();

    // Fill the buffer
    for i in 0..64 {
        assert!(producer.try_push(i as f32).is_ok());
    }

    // Overflow — should not panic, just return Err
    let result = producer.try_push(999.0);
    assert!(result.is_err());
}

/// Validates stereo-to-mono downmix logic (shared across all backends).
#[test]
fn stereo_to_mono_downmix() {
    let stereo_samples: Vec<f32> = vec![
        0.5, 0.3, // Frame 1: L=0.5, R=0.3 → mono=0.4
        1.0, 0.0, // Frame 2: L=1.0, R=0.0 → mono=0.5
        -0.2, 0.6, // Frame 3: L=-0.2, R=0.6 → mono=0.2
    ];

    let channels = 2u32;
    let mut mono_output = Vec::new();

    for chunk in stereo_samples.chunks(channels as usize) {
        let mono = chunk.iter().sum::<f32>() / channels as f32;
        mono_output.push(mono);
    }

    assert_eq!(mono_output.len(), 3);
    assert!((mono_output[0] - 0.4).abs() < f32::EPSILON);
    assert!((mono_output[1] - 0.5).abs() < f32::EPSILON);
    assert!((mono_output[2] - 0.2).abs() < f32::EPSILON);
}
```

**Step 2: Run the tests**

Run: `cargo test --test audio_boundary_test`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add tests/audio_boundary_test.rs
git commit -m "test: add ring buffer contract boundary tests for cross-platform audio"
```

---

### Task 7: Update documentation

**Files:**
- Modify: `CLAUDE.md` (platform constraints section, architecture notes)
- Modify: `Cargo.toml` (keywords if needed)

**Step 1: Update platform constraints in `CLAUDE.md`**

Find the `## Platform Constraints` section and replace it with:

```markdown
## Platform Constraints

- **macOS** — Core Audio `AudioProcessTap` API (requires macOS 15+)
- **Windows** — WASAPI loopback capture via `windows` crate
- **Linux** — PulseAudio monitor source via `libpulse-binding` (works with PipeWire's PulseAudio compat layer)
- Platform-specific deps gated with `[target.'cfg(target_os = "...")'.dependencies]`
- All audio backends export `AudioTap` with same API: `new(producer, config) -> Result<Self>` + `Drop`
- Everything downstream of the ring buffer (FFT, beat detection, UI, visualizations) is cross-platform
```

**Step 2: Update the module map in `CLAUDE.md`**

Find `audio/tap.rs` in the module map and update to:

```markdown
- `audio/tap.rs` — macOS: Core Audio FFI, `AudioTap` lifecycle (all unsafe code lives here)
- `audio/wasapi.rs` — Windows: WASAPI loopback capture, polling capture thread
- `audio/pulse.rs` — Linux: PulseAudio monitor source, blocking read capture thread
```

**Step 3: Update the project description at the top of `CLAUDE.md`**

Change:
```
Terminal-based music visualizer for macOS.
```
To:
```
Terminal-based music visualizer.
```

And update the first sentence to remove "macOS" specificity:
```
Captures system audio via platform-specific APIs (Core Audio on macOS, WASAPI on Windows, PulseAudio on Linux) and renders real-time visualizations using ratatui in the terminal.
```

**Step 4: Verify build one final time**

Run: `cargo build && cargo test --lib && cargo test --test audio_boundary_test`
Expected: all clean

**Step 5: Commit**

```bash
git add CLAUDE.md Cargo.toml
git commit -m "docs: update project docs for cross-platform audio support"
```
