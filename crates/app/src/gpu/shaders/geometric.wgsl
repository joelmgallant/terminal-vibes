// geometric.wgsl — Sacred geometry SDF shapes pulsing with beat

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

const PI: f32 = 3.14159265;

fn sdf_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdf_hex(p: vec2<f32>, r: f32) -> f32 {
    let k = vec2<f32>(-0.866025, 0.5);
    var q = abs(p);
    q -= 2.0 * min(dot(k, q), 0.0) * k;
    return length(q - vec2<f32>(clamp(q.x, -r, r), r)) * sign(q.y - r);
}

fn sdf_triangle(p: vec2<f32>, r: f32) -> f32 {
    let k = sqrt(3.0);
    var q = vec2<f32>(abs(p.x) - r, p.y + r / k);
    if q.x + k * q.y > 0.0 {
        q = vec2<f32>(q.x - k * q.y, -k * q.x - q.y) / 2.0;
    }
    q = vec2<f32>(q.x - clamp(q.x, -2.0 * r, 0.0), q.y);
    return -length(q) * sign(q.y);
}

fn rotate2d(angle: f32) -> mat2x2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return mat2x2<f32>(c, -s, s, c);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (frag_coord.xy - u.resolution * 0.5) / u.resolution.y;
    let screen_uv = frag_coord.xy / u.resolution;

    let rot_speed = u.time * (0.2 + u.mid_energy * 0.3);
    var color = vec3<f32>(0.0);

    // 5 concentric rings of different shapes
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let radius = 0.1 + fi * 0.08;

        // Breathe with beat — each ring offset slightly
        let breathe = radius * (1.0 + u.beat_envelope * 0.15 * (1.0 + fi * 0.2));

        // Rotate each ring differently
        let angle = rot_speed * (1.0 + fi * 0.3) * select(1.0, -1.0, i % 2 == 0);
        let rp = rotate2d(angle) * uv;

        // Alternate shapes: circle, hex, triangle, hex, circle
        var d: f32;
        switch i {
            case 0: { d = sdf_circle(rp, breathe); }
            case 1: { d = sdf_hex(rp, breathe); }
            case 2: { d = sdf_triangle(rp, breathe); }
            case 3: { d = sdf_hex(rp, breathe); }
            default: { d = sdf_circle(rp, breathe); }
        }

        // Ring outline with thickness modulated by beat
        let thickness = 0.008 + u.beat_envelope * 0.004;
        let ring = smoothstep(thickness, 0.0, abs(d));

        // Spectrum-driven brightness per ring
        let freq = fi / 5.0;
        let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq, 0.75)).r;

        // Color per ring layer
        let hue = fi * 0.2 + u.time * 0.05;
        let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
        let p = abs(fract(vec3<f32>(hue) + k) * 6.0 - 3.0);
        let ring_color = mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), 0.7);

        color += ring_color * ring * (0.4 + spectrum * 0.8);
    }

    // Center dot
    let center_glow = 0.003 / (dot(uv, uv) + 0.003);
    color += vec3<f32>(1.0, 0.8, 0.9) * center_glow * (0.2 + u.beat_envelope * 0.3);

    // Subtle outer glow
    let outer = smoothstep(0.5, 0.2, length(uv));
    color *= 0.5 + outer * 0.5;

    // Feedback: subtle trail
    let prev = textureSample(prev_frame, prev_sampler, screen_uv).rgb;
    color = max(color, prev * 0.88);

    return vec4<f32>(color, 1.0);
}
