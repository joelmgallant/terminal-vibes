// --- Binding declarations (same in every viz shader) ---
struct Uniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    frame: u32,
    beat_envelope: f32,
    bass_energy: f32,
    mid_energy: f32,
    treble_energy: f32,
    bass_beat: f32,
    mid_beat: f32,
    treble_beat: f32,
    bpm: f32,
    beat_phase: f32,
    beat_confidence: f32,
    feedback_mix: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;
@group(0) @binding(3) var prev_frame: texture_2d<f32>;
@group(0) @binding(4) var prev_sampler: sampler;

// --- Visualization ---
@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;
    let center = uv - vec2<f32>(0.5, 0.5);
    let aspect = u.resolution.x / u.resolution.y;
    let corrected = vec2<f32>(center.x * aspect, center.y);
    let dist = length(corrected);
    let angle = atan2(corrected.y, corrected.x);

    // Sample spectrum at distance-mapped frequency index
    let freq_uv = clamp(dist * 1.5, 0.0, 1.0);
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq_uv, 0.75)).r;

    // Concentric rings modulated by spectrum amplitude
    let ring_freq = 20.0 + u.bass_energy * 10.0;
    let ring = sin(dist * ring_freq - u.time * 3.0) * 0.5 + 0.5;
    let brightness = ring * spectrum * (0.8 + u.beat_envelope * 0.8);

    // Color rotation based on angle and time
    let hue_shift = u.time * 0.3;
    let r = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift));
    let g = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift + 2.094));
    let b = brightness * (0.5 + 0.5 * sin(angle * 2.0 + hue_shift + 4.189));

    // Vignette
    let vignette = 1.0 - smoothstep(0.3, 0.9, dist);

    return vec4<f32>(r * vignette, g * vignette, b * vignette, 1.0);
}
