// galaxy.wgsl — Spiral galaxy with audio-reactive stars and feedback trails

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

const PI: f32 = 3.14159265;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (frag_coord.xy - u.resolution * 0.5) / u.resolution.y;
    let screen_uv = frag_coord.xy / u.resolution;
    let r = length(uv);
    let angle = atan2(uv.y, uv.x);

    // Rotation driven by mid energy
    let rot_speed = u.time * (0.15 + u.mid_energy * 0.2);

    // Spiral arms
    let spiral_arms = 3.0;
    let spiral = sin(angle * spiral_arms - r * 8.0 + rot_speed) * 0.5 + 0.5;
    let arm_width = spiral * exp(-r * 2.0);

    // Stars from noise
    let star_uv = uv * 30.0 + vec2<f32>(rot_speed * 0.3);
    let star_noise = hash21(floor(star_uv));
    let star_threshold = 0.92 - u.treble_energy * 0.05;
    let star = select(0.0, star_noise, star_noise > star_threshold);
    let star_brightness = star * (0.5 + u.beat_envelope * 0.5);

    // Spectrum modulates arm brightness
    let freq = clamp(r * 2.0, 0.0, 1.0);
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq, 0.75)).r;

    // Core glow
    let core_glow = 0.02 / (r * r + 0.02);
    let core_color = vec3<f32>(0.9, 0.7, 1.0) * core_glow * (0.3 + u.beat_envelope * 0.4);

    // Arm color: blue-purple-pink based on angle and distance
    let hue = angle / (2.0 * PI) + 0.6 + r * 0.1;
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(hue) + k) * 6.0 - 3.0);
    let arm_color = mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), 0.7);

    var color = arm_color * arm_width * (spectrum * 0.6 + 0.2) * (0.5 + u.beat_envelope * 0.5);
    color += core_color;
    color += vec3<f32>(0.8, 0.9, 1.0) * star_brightness;

    // Bass beat: brightness wave rippling outward
    let beat_wave = sin(r * 20.0 - u.time * 8.0) * 0.5 + 0.5;
    color += vec3<f32>(0.3, 0.1, 0.5) * beat_wave * u.bass_beat * 0.3;

    // Feedback: star trails with slight rotation
    let rot_amount = 0.003;
    let cs = cos(rot_amount);
    let sn = sin(rot_amount);
    let rot_uv = (screen_uv - 0.5) * mat2x2<f32>(cs, -sn, sn, cs) + 0.5;
    let prev = textureSample(prev_frame, prev_sampler, rot_uv).rgb;
    color = max(color, prev * 0.93);

    return vec4<f32>(color, 1.0);
}
