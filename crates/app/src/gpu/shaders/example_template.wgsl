// example_template.wgsl — Template for custom terminal-vibes shaders
//
// Save .wgsl files to ~/.config/terminal-vibes/shaders/
// They will be automatically discovered and hot-reloaded.
//
// Available inputs:
//   audio_data (texture): 256x2 RGBA texture
//     - Row 0 (sample at y=0.25): waveform (PCM samples, 0..1)
//     - Row 1 (sample at y=0.75): FFT spectrum (frequency bands, 0..1)
//   u.time: seconds since start
//   u.delta_time: seconds since last frame
//   u.resolution: window size in pixels
//   u.beat_envelope: overall beat envelope (0..1, fast attack, slow decay)
//   u.bass_energy, u.mid_energy, u.treble_energy: frequency band energy
//   u.bass_beat, u.mid_beat, u.treble_beat: 1.0 on beat, 0.0 otherwise
//   u.bpm: estimated beats per minute
//   u.beat_phase: 0..1 phase within beat cycle
//   prev_frame (texture): previous frame output (for feedback effects)
//   u.feedback_mix: suggested feedback blend factor (0..1, default 0.95)

struct Uniforms {
    time: f32, delta_time: f32, resolution: vec2<f32>, frame: u32,
    beat_envelope: f32, bass_energy: f32, mid_energy: f32, treble_energy: f32,
    bass_beat: f32, mid_beat: f32, treble_beat: f32,
    bpm: f32, beat_phase: f32, beat_confidence: f32, feedback_mix: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;
@group(0) @binding(3) var prev_frame: texture_2d<f32>;
@group(0) @binding(4) var prev_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(frag_coord.x, u.resolution.y - frag_coord.y) / u.resolution;

    // Your visualization here!
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.75)).r;
    let brightness = spectrum * (0.5 + u.beat_envelope * 0.5);

    return vec4<f32>(uv.x * brightness, uv.y * brightness, brightness, 1.0);
}
