use terminal_vibes::beat::{BeatDetectionConfig, BeatDetector};

fn rhythmic_bass_pattern(num_bands: usize, loud: bool) -> Vec<f32> {
    let mut spectrum = vec![0.05; num_bands];
    if loud {
        for i in 0..37 {
            spectrum[i] = 0.9;
        }
    }
    spectrum
}

#[test]
fn test_rhythmic_pattern_detects_beats() {
    let config = BeatDetectionConfig::default();
    let mut detector = BeatDetector::new(128, config);

    let quiet = rhythmic_bass_pattern(128, false);
    for _ in 0..50 {
        detector.analyze(&quiet);
    }

    let loud = rhythmic_bass_pattern(128, true);
    let mut beats_detected = 0;

    for _ in 0..4 {
        let result = detector.analyze(&loud);
        if result.bass_beat {
            beats_detected += 1;
        }
        for _ in 0..10 {
            detector.analyze(&quiet);
        }
    }

    assert!(
        beats_detected >= 3,
        "Should detect at least 3 of 4 rhythmic beats, got {}",
        beats_detected
    );
}

#[test]
fn test_steady_signal_does_not_beat_continuously() {
    let config = BeatDetectionConfig::default();
    let mut detector = BeatDetector::new(128, config);

    let loud = vec![0.8_f32; 128];
    let mut beat_count = 0;

    for _ in 0..100 {
        let result = detector.analyze(&loud);
        if result.beat {
            beat_count += 1;
        }
    }

    assert!(
        beat_count < 10,
        "Steady signal should not produce continuous beats, got {}",
        beat_count
    );
}
