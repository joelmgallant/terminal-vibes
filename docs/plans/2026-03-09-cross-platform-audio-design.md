# Cross-Platform Audio Support Design

## Goal

Add Windows and Linux audio capture support alongside the existing macOS backend. All three platforms capture system audio loopback (what's playing out of the speakers) and feed it into the same ring buffer pipeline.

## Architecture

### Module Structure

```
src/audio/
  mod.rs          # AudioConfig + cfg-gated module imports
  tap.rs          # macOS: Core Audio ProcessTap (existing, unchanged)
  wasapi.rs       # Windows: WASAPI loopback via `windows` crate
  pulse.rs        # Linux: PulseAudio monitor source via `libpulse-binding`
```

### Platform Abstraction

Conditional module swap — no trait needed. Each platform file exports `AudioTap` with the same public API:

- `pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self>`
- `impl Drop` — cleanup on drop
- `unsafe impl Send` — movable across threads
- No methods called after construction — "hold it alive" lifecycle

`mod.rs` uses `#[cfg(target_os = "...")]` to swap which module is compiled and re-exported. Zero changes to `main.rs`, `processing.rs`, or anything downstream.

### Downstream Impact

None. The ring buffer (`HeapProd<f32>`) is the contract boundary. Everything after it — FFT pipeline, beat detection, UI, visualizations — is already 100% cross-platform pure Rust.

## Windows: WASAPI Loopback

### Approach

Raw WASAPI FFI via Microsoft's `windows` crate. Loopback mode captures the default audio output device.

### Lifecycle

1. `IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eConsole)` — get default speakers
2. `IAudioClient::Initialize()` with `AUDCLNT_STREAMFLAGS_LOOPBACK` — open loopback stream
3. `IAudioClient::GetService::<IAudioCaptureClient>()` — get capture interface
4. Spawn capture thread — polls `GetBuffer()` ~every 10ms, pushes f32 samples into ring buffer with stereo-to-mono downmix
5. `IAudioClient::Start()` — begin capture
6. Drop: `Stop()`, release COM objects, join capture thread

### Dependencies

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_Devices_Properties",
] }
```

### Notes

- WASAPI is poll-based (not callback), so we spawn our own capture thread
- Loopback typically gives f32 PCM at the device's native sample rate
- Pass actual sample rate through `AudioConfig` rather than resampling

## Linux: PulseAudio Monitor Source

### Approach

PulseAudio's Simple API with monitor source capture. Works on both PulseAudio and PipeWire's compatibility layer.

### Lifecycle

1. Create `Simple` connection targeting `@DEFAULT_SINK@.monitor`
2. Configure: f32 format, stereo, 44100 Hz (PulseAudio handles resampling)
3. Spawn capture thread — blocking `simple.read()` loop, pushes samples into ring buffer with stereo-to-mono downmix
4. Drop: disconnect, join capture thread

### Dependencies

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libpulse-binding = "2.28"
libpulse-simple-binding = "2.28"
```

### System Requirements

- `libpulse-dev` (Debian/Ubuntu) or `pulseaudio-libs-devel` (Fedora)
- PulseAudio or PipeWire with PulseAudio compatibility layer running

## Error Handling

### Startup

- No audio device → descriptive error, clean exit
- Permission denied → message telling user what to grant
- Linux: PulseAudio not running → "Could not connect to PulseAudio server"

### Runtime

- Device disconnected → capture thread stops pushing, visualizations go flat, no crash
- Ring buffer full → `try_push()` drops samples silently (existing behavior)
- Sample rate mismatch (Windows) → pass actual rate via `AudioConfig`, processing adapts

### Explicitly Out of Scope (YAGNI)

- No device selection UI — default output device only
- No hot-swap on device change — user restarts app
- No resampling on Windows — use native device rate
- No ALSA fallback on Linux

## Testing

### Platform Integration Tests

- `tests/audio_capture_test.rs` — construct `AudioTap`, verify no panic
- `#[cfg(target_os = "...")]` gated per platform
- `#[ignore]` by default (needs real audio hardware)

### Contract Boundary Tests

- `tests/audio_mock_test.rs` — push known samples into ring buffer, verify processing pipeline output
- Platform-independent, runs everywhere

### CI Matrix

- Add `windows-latest` and `ubuntu-latest` to GitHub Actions
- `cargo build` + `cargo test --lib` + `cargo clippy` on all three platforms
- Existing `macos-latest` stays for release builds
