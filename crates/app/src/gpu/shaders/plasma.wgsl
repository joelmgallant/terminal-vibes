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
    _pad: f32,
};

@group(0) @binding(0) var audio_data: texture_2d<f32>;
@group(0) @binding(1) var audio_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;
    let t = u.time;
    let aspect = u.resolution.x / u.resolution.y;
    let p = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);

    let bass = u.bass_energy;
    let mid = u.mid_energy;
    let treble = u.treble_energy;

    var v = 0.0;
    v += sin(p.x * 10.0 + t * 1.2 + bass * 6.0);
    v += sin(p.y * 12.0 + t * 0.8 + mid * 6.0);
    v += sin((p.x + p.y) * 8.0 + t * 0.6);
    v += sin(length(p) * 14.0 - t * 2.0 + treble * 6.0);
    v += sin(length(p - vec2<f32>(0.3 * sin(t * 0.5), 0.2 * cos(t * 0.7))) * 12.0);
    v *= 0.2;

    let r = sin(v * 3.14159 + t * 0.3) * 0.5 + 0.5;
    let g = sin(v * 3.14159 + t * 0.3 + 2.094) * 0.5 + 0.5;
    let b = sin(v * 3.14159 + t * 0.3 + 4.189) * 0.5 + 0.5;

    let brightness = 0.6 + u.beat_envelope * 0.4;

    return vec4<f32>(r * brightness, g * brightness, b * brightness, 1.0);
}
