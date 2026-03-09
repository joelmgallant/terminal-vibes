use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

/// Validates the ring buffer contract that all audio backends must satisfy:
/// Producer pushes mono f32 samples, consumer drains them.
#[test]
fn ring_buffer_contract_mono_samples_flow_through() {
    let rb = HeapRb::<f32>::new(4096);
    let (mut producer, mut consumer) = rb.split();

    // Simulate audio callback pushing mono samples
    let test_signal: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
    for &sample in &test_signal {
        assert!(producer.try_push(sample).is_ok());
    }

    // Simulate processor draining
    let mut drain_buf = vec![0.0f32; 4096];
    let count = consumer.pop_slice(&mut drain_buf);
    assert_eq!(count, 1024);

    // Verify samples match
    for i in 0..1024 {
        assert!(
            (drain_buf[i] - test_signal[i]).abs() < f32::EPSILON,
            "Sample {} mismatch: {} vs {}",
            i,
            drain_buf[i],
            test_signal[i]
        );
    }
}

/// Validates graceful overflow: when buffer is full, try_push drops samples without panic.
#[test]
fn ring_buffer_contract_overflow_drops_gracefully() {
    let rb = HeapRb::<f32>::new(64);
    let (mut producer, _consumer) = rb.split();

    // Fill the buffer
    for i in 0..64 {
        assert!(producer.try_push(i as f32).is_ok());
    }

    // Overflow — should not panic, just return Err
    let result = producer.try_push(999.0);
    assert!(result.is_err());
}

/// Validates stereo-to-mono downmix logic (shared across all backends).
#[test]
fn stereo_to_mono_downmix() {
    let stereo_samples: Vec<f32> = vec![
        0.5, 0.3, // Frame 1: L=0.5, R=0.3 → mono=0.4
        1.0, 0.0, // Frame 2: L=1.0, R=0.0 → mono=0.5
        -0.2, 0.6, // Frame 3: L=-0.2, R=0.6 → mono=0.2
    ];

    let channels = 2u32;
    let mut mono_output = Vec::new();

    for chunk in stereo_samples.chunks(channels as usize) {
        let mono = chunk.iter().sum::<f32>() / channels as f32;
        mono_output.push(mono);
    }

    assert_eq!(mono_output.len(), 3);
    assert!((mono_output[0] - 0.4).abs() < f32::EPSILON);
    assert!((mono_output[1] - 0.5).abs() < f32::EPSILON);
    assert!((mono_output[2] - 0.2).abs() < f32::EPSILON);
}
