// fractal.wgsl — Julia set with audio-reactive parameters

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
    let aspect = u.resolution.x / u.resolution.y;
    let uv = (frag_coord.xy / u.resolution - 0.5) * 2.0;
    let p = vec2<f32>(uv.x * aspect, uv.y);

    // Zoom oscillates gently
    let zoom = 1.5 + 0.5 * sin(u.time * 0.15);
    var z = p * zoom;

    // Julia set c parameter modulated by audio
    let base_angle = u.time * 0.2;
    let cr = -0.7 + u.bass_energy * 0.3 * cos(base_angle);
    let ci = 0.27015 + u.mid_energy * 0.2 * sin(base_angle * 1.3);
    let c = vec2<f32>(cr, ci);

    // Iterate
    var iter = 0;
    let max_iter = 80;
    for (var i = 0; i < max_iter; i++) {
        if dot(z, z) > 4.0 { break; }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        iter = i + 1;
    }

    // Smooth iteration count
    let smooth_iter = f32(iter) - log2(log2(dot(z, z))) + 4.0;
    let t = smooth_iter / f32(max_iter);

    // Color from iteration count
    let hue = t * 3.0 + u.time * 0.1;
    let sat = 0.8;
    let val = select(1.0 - t * 0.3, 0.0, iter >= max_iter);

    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let rgb_p = abs(fract(vec3<f32>(hue) + k) * 6.0 - 3.0);
    var color = val * mix(vec3<f32>(1.0), clamp(rgb_p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), sat);

    // Beat brightness pulse
    color *= 0.7 + u.beat_envelope * 0.5;

    // Beat flash on bass
    color += vec3<f32>(u.bass_beat * 0.1);

    return vec4<f32>(color, 1.0);
}
