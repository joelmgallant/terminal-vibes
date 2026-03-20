// neon_landscape.wgsl — Synthwave grid with spectrum mountains and sun

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
    let uv = frag_coord.xy / u.resolution;
    let aspect = u.resolution.x / u.resolution.y;

    // Sky gradient (top half)
    let horizon = 0.45;
    var color = vec3<f32>(0.0);

    if uv.y > horizon {
        // Sky
        let sky_t = (uv.y - horizon) / (1.0 - horizon);
        let sky = mix(
            vec3<f32>(0.1, 0.0, 0.2),   // horizon: dark purple
            vec3<f32>(0.02, 0.0, 0.08),  // zenith: near black
            sky_t
        );
        color = sky;

        // Sun
        let sun_center = vec2<f32>(0.5, horizon + 0.2);
        let sun_uv = vec2<f32>((uv.x - sun_center.x) * aspect, uv.y - sun_center.y);
        let sun_dist = length(sun_uv);
        let sun_radius = 0.12 + u.beat_envelope * 0.03;

        if sun_dist < sun_radius {
            // Scanlines through sun
            let scanline = step(0.5, fract(uv.y * 40.0 - u.time * 0.5));
            let sun_color = mix(
                vec3<f32>(1.0, 0.3, 0.5),
                vec3<f32>(1.0, 0.9, 0.2),
                (uv.y - horizon) / 0.4
            );
            color = sun_color * (0.8 + scanline * 0.2);
        }
        // Sun glow
        let glow = 0.03 / (sun_dist * sun_dist + 0.03);
        color += vec3<f32>(1.0, 0.2, 0.6) * glow * 0.15;

        // Mountains from spectrum
        let mx = uv.x;
        let spectrum = textureSample(audio_data, audio_sampler, vec2<f32>(mx, 0.75)).r;
        let mountain_height = horizon + spectrum * 0.25 + 0.02;
        if uv.y < mountain_height {
            let mt = (mountain_height - uv.y) / 0.25;
            color = mix(vec3<f32>(0.15, 0.0, 0.3), vec3<f32>(0.05, 0.0, 0.1), mt);
        }
    } else {
        // Ground: perspective grid
        let gy = horizon - uv.y;
        let depth = 0.1 / (gy + 0.001);
        let gx = (uv.x - 0.5) * depth * aspect;

        // Scrolling grid
        let scroll = u.time * 2.0 + u.bass_energy * 3.0;
        let grid_x = abs(fract(gx * 0.5) - 0.5);
        let grid_y = abs(fract(depth * 0.3 + scroll) - 0.5);
        let line_x = smoothstep(0.02, 0.0, grid_x);
        let line_y = smoothstep(0.02, 0.0, grid_y);
        let grid = max(line_x, line_y);

        // Grid color: cyan with treble brightness
        let grid_brightness = grid * (0.4 + u.treble_energy * 0.6 + u.beat_envelope * 0.3);
        color = vec3<f32>(0.0, grid_brightness * 0.8, grid_brightness);

        // Distance fade
        let fade = exp(-gy * 8.0);
        color *= (1.0 - fade) + fade * 0.3;

        // Horizon glow
        let horizon_glow = exp(-gy * 30.0);
        color += vec3<f32>(1.0, 0.1, 0.6) * horizon_glow * 0.4;
    }

    return vec4<f32>(color, 1.0);
}
