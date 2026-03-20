// aurora.wgsl — Northern lights curtains driven by audio

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

fn fbm(p: vec2<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i++) {
        val += amp * noise2d(pos);
        pos *= 2.1;
        amp *= 0.5;
    }
    return val;
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;
    let aspect = u.resolution.x / u.resolution.y;
    let p = vec2<f32>(uv.x * aspect, uv.y);

    let t = u.time * 0.3;

    // Aurora curtains: layered horizontal waves
    var aurora = 0.0;
    let spectrum_low = textureSample(audio_data, audio_sampler, vec2<f32>(0.1, 0.75)).r;
    let spectrum_mid = textureSample(audio_data, audio_sampler, vec2<f32>(0.4, 0.75)).r;
    let spectrum_high = textureSample(audio_data, audio_sampler, vec2<f32>(0.7, 0.75)).r;

    // Curtain 1: wide, bass-driven
    let wave1_y = 0.55 + sin(p.x * 2.0 + t * 1.2) * 0.08 + u.bass_energy * 0.05;
    let curtain1 = exp(-pow(abs(p.y - wave1_y) * 6.0, 2.0)) * (0.5 + spectrum_low * 0.8);

    // Curtain 2: mid-frequency driven, thinner
    let wave2_y = 0.6 + sin(p.x * 3.5 + t * 0.8 + 1.0) * 0.06 + u.mid_energy * 0.04;
    let curtain2 = exp(-pow(abs(p.y - wave2_y) * 8.0, 2.0)) * (0.4 + spectrum_mid * 0.7);

    // Curtain 3: treble shimmer
    let wave3_y = 0.65 + sin(p.x * 5.0 + t * 1.5 + 2.5) * 0.04 + u.treble_energy * 0.03;
    let curtain3 = exp(-pow(abs(p.y - wave3_y) * 10.0, 2.0)) * (0.3 + spectrum_high * 0.6);

    // Noise shimmer
    let shimmer = fbm(p * vec2<f32>(4.0, 8.0) + vec2<f32>(t * 0.5, t * 0.3));

    // Aurora palette: green → teal → purple → pink
    let c1_color = vec3<f32>(0.1, 0.9, 0.3) * curtain1;  // green
    let c2_color = vec3<f32>(0.0, 0.7, 0.8) * curtain2;  // teal
    let c3_color = vec3<f32>(0.6, 0.2, 0.8) * curtain3;  // purple

    var color = c1_color + c2_color + c3_color;
    color *= (0.7 + shimmer * 0.5);
    color *= 0.6 + u.beat_envelope * 0.6;

    // Stars in the dark sky
    let star_uv = uv * 50.0;
    let star = hash21(floor(star_uv));
    let star_twinkle = sin(star * 100.0 + u.time * 3.0) * 0.5 + 0.5;
    if star > 0.985 && uv.y > 0.5 {
        color += vec3<f32>(0.8) * star_twinkle * (1.0 - (curtain1 + curtain2 + curtain3));
    }

    // Dark ground gradient
    let ground = smoothstep(0.2, 0.0, uv.y);
    color = mix(color, vec3<f32>(0.01, 0.01, 0.03), ground);

    return vec4<f32>(color, 1.0);
}
