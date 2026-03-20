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

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3<f32>(h, h, h) + k) * 6.0 - vec3<f32>(3.0, 3.0, 3.0));
    return v * mix(vec3<f32>(1.0, 1.0, 1.0), clamp(p - vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / u.resolution;

    // Top half: spectrum bars
    if uv.y > 0.5 {
        let bar_y = (uv.y - 0.5) * 2.0;
        let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.75)).r;
        if bar_y < spectrum {
            let color = hsv2rgb(uv.x * 0.8, 0.8, 1.0 - bar_y * 0.3);
            return vec4<f32>(color * (0.8 + u.beat_envelope * 0.4), 1.0);
        }
        return vec4<f32>(0.02, 0.02, 0.05, 1.0);
    }

    // Bottom half: waveform with glow
    let waveform = textureSample(audio_data, audio_sampler, vec2<f32>(uv.x, 0.25)).r;
    let wave_y = uv.y * 2.0;
    let wave_pos = waveform * 0.4 + 0.5;
    let dist = abs(wave_y - wave_pos);
    let glow = 0.004 / (dist * dist + 0.004);
    let color = vec3<f32>(0.2, 0.8, 0.4) * glow * (0.6 + u.beat_envelope * 0.5);

    return vec4<f32>(color, 1.0);
}
