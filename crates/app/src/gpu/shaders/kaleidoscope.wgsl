// kaleidoscope.wgsl — Angular symmetry with feedback mandala trails

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
const TAU: f32 = 6.28318530;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (frag_coord.xy - u.resolution * 0.5) / u.resolution.y;
    let screen_uv = frag_coord.xy / u.resolution;
    let r = length(uv);
    var angle = atan2(uv.y, uv.x) + PI;

    // Rotate over time, speed from mid energy
    angle += u.time * (0.3 + u.mid_energy * 0.5);

    // Segment count snaps on bass beat (4, 6, or 8)
    let seg_base = 6.0 + u.bass_energy * 4.0;
    let segments = floor(seg_base / 2.0) * 2.0; // even numbers only
    let seg_angle = TAU / segments;

    // Kaleidoscope fold — use manual modulo instead of % on floats
    var a = angle - floor(angle / seg_angle) * seg_angle;
    if a > seg_angle * 0.5 {
        a = seg_angle - a;
    }
    let kp = vec2<f32>(cos(a), sin(a)) * r;

    // Sample spectrum at folded position
    let freq = clamp(r * 1.5, 0.0, 1.0);
    let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(freq, 0.75)).r;

    // Base pattern: layered sine waves
    var pattern = 0.0;
    pattern += sin(kp.x * 12.0 + u.time * 1.5) * 0.5 + 0.5;
    pattern += sin(kp.y * 8.0 - u.time * 1.1) * 0.5 + 0.5;
    pattern += sin((kp.x + kp.y) * 15.0 + u.time) * 0.3;
    pattern *= 0.33;

    let brightness = (pattern * spectrum + spectrum * 0.3) * (0.6 + u.beat_envelope * 0.7);

    // Color from angle and distance
    let hue = a / seg_angle + r * 0.3 + u.time * 0.08;
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(hue) + k) * 6.0 - 3.0);
    var color = brightness * mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), 0.8);

    // Feedback: mandala trail
    let prev = textureSample(prev_frame, prev_sampler, screen_uv).rgb;
    // Slight zoom on feedback for spiral effect
    let zoom_uv = (screen_uv - 0.5) * 0.99 + 0.5;
    let prev_zoomed = textureSample(prev_frame, prev_sampler, zoom_uv).rgb;
    color = max(color, prev_zoomed * 0.92);

    // Vignette
    let vig = 1.0 - smoothstep(0.4, 0.9, r);
    color *= vig;

    return vec4<f32>(color, 1.0);
}
