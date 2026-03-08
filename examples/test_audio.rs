//! Quick test to verify audio capture works without TUI.
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Split};
use std::thread;
use std::time::Duration;

/// Query and log stream info for an audio device.
fn log_device_streams(device_id: u32) {
    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    extern "C" {
        fn AudioObjectGetPropertyDataSize(
            object_id: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            data_size: *mut u32,
        ) -> i32;

        fn AudioObjectGetPropertyData(
            object_id: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            data_size: *mut u32,
            data: *mut std::ffi::c_void,
        ) -> i32;
    }

    let scope_input: u32 = u32::from_be_bytes(*b"inpt");
    let scope_output: u32 = u32::from_be_bytes(*b"outp");
    let selector_streams: u32 = u32::from_be_bytes(*b"stm#");

    for (scope_name, scope) in [("input", scope_input), ("output", scope_output)] {
        let address = AudioObjectPropertyAddress {
            selector: selector_streams,
            scope,
            element: 0,
        };
        let mut data_size: u32 = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                device_id,
                &address,
                0,
                std::ptr::null(),
                &mut data_size,
            )
        };
        let num_streams = data_size as usize / std::mem::size_of::<u32>();
        eprintln!(
            "  Device {} {} streams: count={}, status={}",
            device_id, scope_name, num_streams, status
        );
    }
}

fn main() {
    env_logger::init();

    let rb = HeapRb::<f32>::new(44100 * 2);
    let (producer, mut consumer) = rb.split();

    let config = terminal_vibes::audio::AudioConfig {
        sample_rate: 44100.0,
        buffer_size: 4096,
        channels: 2,
    };

    eprintln!("Creating audio tap...");
    let tap = match terminal_vibes::audio::AudioTap::new(producer, config) {
        Ok(tap) => {
            eprintln!("Audio tap created successfully!");
            tap
        }
        Err(e) => {
            eprintln!("Failed to create audio tap: {}", e);
            std::process::exit(1);
        }
    };

    // Query aggregate device streams
    eprintln!("\nQuerying aggregate device streams:");
    log_device_streams(tap.aggregate_device_id());

    eprintln!("\nListening for 5 seconds... play some audio!");

    let mut total_samples = 0u64;
    let mut max_amplitude: f32 = 0.0;
    let mut buf = vec![0.0f32; 4096];

    for i in 0..50 {
        thread::sleep(Duration::from_millis(100));
        let count = consumer.pop_slice(&mut buf);
        total_samples += count as u64;

        for &s in &buf[..count] {
            let abs = s.abs();
            if abs > max_amplitude {
                max_amplitude = abs;
            }
        }

        if i % 10 == 9 {
            eprintln!(
                "[{:.1}s] samples received: {}, max amplitude: {:.6}",
                (i + 1) as f64 / 10.0,
                total_samples,
                max_amplitude
            );
        }
    }

    eprintln!("\nDone! Total samples: {}, Max amplitude: {:.6}", total_samples, max_amplitude);
    if total_samples > 0 && max_amplitude > 0.0001 {
        eprintln!("Audio capture is WORKING!");
    } else if total_samples > 0 {
        eprintln!("Samples received but amplitude very low. Is audio playing?");
    } else {
        eprintln!("WARNING: No samples received.");
    }

    drop(tap);
}
