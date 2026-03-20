// tunnel.wgsl — Barrel-distortion tunnel with audio-reactive walls

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

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(h, h, h) + k) * 6.0 - 3.0);
    return v * mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (frag_coord.xy - u.resolution * 0.5) / u.resolution.y;
    let r = length(uv);
    let angle = atan2(uv.y, uv.x) / 3.14159;

    // Tunnel mapping: depth from inverse radius
    let depth = 0.5 / (r + 0.001);

    // Scroll forward, speed driven by bass
    let speed = u.time * (1.5 + u.bass_energy * 2.0);
    let tx = angle + u.time * 0.08;
    let ty = depth + speed;

    // Sample spectrum at angular position
    let freq_uv = abs(angle) * 0.5 + 0.25;
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq_uv, 0.75)).r;

    // Wall pattern: rings modulated by treble
    let rings = sin(ty * 12.0 + u.treble_energy * 8.0) * 0.5 + 0.5;
    let grid = sin(tx * 16.0) * 0.5 + 0.5;
    let pattern = rings * 0.7 + grid * spectrum * 0.3;

    // Brightness with beat pulse
    let brightness = pattern * (0.5 + u.beat_envelope * 0.8);

    // Color rotation
    let hue = tx * 0.3 + u.time * 0.1 + depth * 0.02;
    var color = hsv2rgb(hue, 0.75, brightness);

    // Beat flash
    color += vec3<f32>(u.bass_beat * 0.15 * spectrum);

    // Depth fog and center vignette
    let fog = 1.0 - exp(-depth * 0.08);
    let center_mask = smoothstep(0.0, 0.15, r);

    color *= fog * center_mask;

    // Optional feedback trail
    let prev = textureSample(prev_frame, prev_sampler, frag_coord.xy / u.resolution).rgb;
    color = max(color, prev * u.feedback_mix);

    return vec4<f32>(color, 1.0);
}
