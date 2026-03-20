// fluid.wgsl — Feedback-driven smoke simulation

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

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let sm = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), sm.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), sm.x),
        sm.y
    );
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(frag_coord.x, u.resolution.y - frag_coord.y) / u.resolution;
    let centered = uv - 0.5;

    // Velocity field from spectrum-driven noise
    let noise_scale = 3.0 + u.bass_energy * 2.0;
    let t = u.time * 0.5;
    let vel_x = noise2d(uv * noise_scale + vec2<f32>(t, 0.0)) - 0.5;
    let vel_y = noise2d(uv * noise_scale + vec2<f32>(0.0, t + 100.0)) - 0.5;
    let velocity = vec2<f32>(vel_x, vel_y) * 0.008 * (1.0 + u.beat_envelope * 2.0);

    // Read previous frame with distortion
    let distorted_uv = uv + velocity;
    let prev = textureSample(prev_frame, prev_sampler, distorted_uv).rgb;

    // Dissipation: slight darkening each frame
    var color = prev * u.feedback_mix;

    // Slight upward drift (heat rises)
    let drift_uv = uv + vec2<f32>(0.0, 0.002);
    let drifted = textureSample(prev_frame, prev_sampler, drift_uv).rgb;
    color = max(color, drifted * (u.feedback_mix - 0.01));

    // Color injection based on audio
    let dist_center = length(centered);

    // Bass: inject red/orange from center
    if dist_center < 0.15 + u.bass_energy * 0.1 {
        let inject = u.bass_energy * 0.3 * (1.0 - dist_center * 5.0);
        color += vec3<f32>(inject * 1.0, inject * 0.3, inject * 0.05);
    }

    // Mid: inject green/cyan from sides
    let side_dist = abs(centered.x);
    if side_dist > 0.3 && abs(centered.y) < 0.15 {
        let inject = u.mid_energy * 0.2;
        color += vec3<f32>(inject * 0.1, inject * 0.6, inject * 0.5);
    }

    // Treble: inject blue/purple sparkles
    let sparkle_uv = uv * 20.0 + vec2<f32>(u.time * 2.0);
    let sparkle = hash21(floor(sparkle_uv));
    if sparkle > 0.97 && u.treble_energy > 0.3 {
        color += vec3<f32>(0.3, 0.1, 0.8) * u.treble_energy * 0.5;
    }

    // Beat flash: burst from center
    if u.bass_beat > 0.5 {
        let burst = exp(-dist_center * 8.0) * 0.4;
        color += vec3<f32>(1.0, 0.5, 0.2) * burst;
    }

    // Clamp to prevent overflow
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
