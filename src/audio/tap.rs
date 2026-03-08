use super::AudioConfig;
use anyhow::{anyhow, Result};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;

// Core Audio FFI types and constants
#[allow(non_camel_case_types)]
mod ffi {
    use std::os::raw::c_void;

    pub type OSStatus = i32;
    pub type AudioObjectID = u32;
    pub type AudioDeviceID = AudioObjectID;
    pub type UInt32 = u32;
    pub type Float64 = f64;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;

    // AudioObjectPropertyAddress
    #[repr(C)]
    pub struct AudioObjectPropertyAddress {
        pub selector: u32,
        pub scope: u32,
        pub element: u32,
    }

    // Property selectors
    pub const kAudioHardwarePropertyProcessTapList: u32 = u32::from_be_bytes(*b"tps#");
    pub const kAudioHardwarePropertyTapList: u32 = u32::from_be_bytes(*b"tps#");

    // Scopes
    pub const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    pub const kAudioObjectPropertyScopeInput: u32 = u32::from_be_bytes(*b"inpt");
    pub const kAudioObjectPropertyScopeOutput: u32 = u32::from_be_bytes(*b"outp");
    pub const kAudioObjectPropertyElementMain: u32 = 0;

    // Device properties
    pub const kAudioDevicePropertyStreamConfiguration: u32 = u32::from_be_bytes(*b"slay");
    pub const kAudioDevicePropertyNominalSampleRate: u32 = u32::from_be_bytes(*b"nsrt");

    // AudioUnit types
    pub type AudioUnit = *mut c_void;
    pub type AudioComponentInstance = AudioUnit;
    pub type AudioComponent = *mut c_void;

    #[repr(C)]
    pub struct AudioComponentDescription {
        pub component_type: u32,
        pub component_sub_type: u32,
        pub component_manufacturer: u32,
        pub component_flags: u32,
        pub component_flags_mask: u32,
    }

    pub const kAudioUnitType_Output: u32 = u32::from_be_bytes(*b"auou");
    pub const kAudioUnitSubType_HALOutput: u32 = u32::from_be_bytes(*b"ahal");
    pub const kAudioUnitManufacturer_Apple: u32 = u32::from_be_bytes(*b"appl");

    // AudioUnit properties
    pub const kAudioOutputUnitProperty_EnableIO: u32 = 2003;
    pub const kAudioOutputUnitProperty_CurrentDevice: u32 = 2000;
    pub const kAudioUnitProperty_StreamFormat: u32 = 8;
    pub const kAudioUnitProperty_SetRenderCallback: u32 = 23;

    pub const kAudioUnitScope_Input: u32 = 1;
    pub const kAudioUnitScope_Output: u32 = 0;
    pub const kAudioUnitScope_Global: u32 = 0;

    #[repr(C)]
    pub struct AudioStreamBasicDescription {
        pub sample_rate: Float64,
        pub format_id: u32,
        pub format_flags: u32,
        pub bytes_per_packet: u32,
        pub frames_per_packet: u32,
        pub bytes_per_frame: u32,
        pub channels_per_frame: u32,
        pub bits_per_channel: u32,
        pub reserved: u32,
    }

    pub const kAudioFormatLinearPCM: u32 = u32::from_be_bytes(*b"lpcm");
    pub const kAudioFormatFlagIsFloat: u32 = 1 << 0;
    pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;
    pub const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

    #[repr(C)]
    pub struct AudioBufferList {
        pub number_buffers: u32,
        pub buffers: [AudioBuffer; 1], // variable-length array
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub number_channels: u32,
        pub data_byte_size: u32,
        pub data: *mut c_void,
    }

    #[repr(C)]
    pub struct AURenderCallbackStruct {
        pub input_proc: unsafe extern "C" fn(
            in_ref_con: *mut c_void,
            io_action_flags: *mut u32,
            in_time_stamp: *const AudioTimeStamp,
            in_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut AudioBufferList,
        ) -> OSStatus,
        pub input_proc_ref_con: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioTimeStamp {
        pub sample_time: f64,
        pub host_time: u64,
        pub rate_scalar: f64,
        pub word_clock_time: u64,
        pub smpte_time: [u8; 24], // SMPTETime struct, opaque here
        pub flags: u32,
        pub reserved: u32,
    }

    // CATapDescription for macOS 15+
    // This is an Objective-C class. We interact via objc runtime.

    extern "C" {
        pub fn AudioObjectGetPropertyDataSize(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            out_data_size: *mut u32,
        ) -> OSStatus;

        pub fn AudioObjectGetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            io_data_size: *mut u32,
            out_data: *mut c_void,
        ) -> OSStatus;

        pub fn AudioObjectSetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            data_size: u32,
            data: *const c_void,
        ) -> OSStatus;

        pub fn AudioComponentFindNext(
            component: AudioComponent,
            description: *const AudioComponentDescription,
        ) -> AudioComponent;

        pub fn AudioComponentInstanceNew(
            component: AudioComponent,
            out_instance: *mut AudioComponentInstance,
        ) -> OSStatus;

        pub fn AudioComponentInstanceDispose(instance: AudioComponentInstance) -> OSStatus;

        pub fn AudioUnitSetProperty(
            unit: AudioUnit,
            property_id: u32,
            scope: u32,
            element: u32,
            data: *const c_void,
            data_size: u32,
        ) -> OSStatus;

        pub fn AudioUnitGetProperty(
            unit: AudioUnit,
            property_id: u32,
            scope: u32,
            element: u32,
            data: *mut c_void,
            data_size: *mut u32,
        ) -> OSStatus;

        pub fn AudioUnitInitialize(unit: AudioUnit) -> OSStatus;
        pub fn AudioUnitUninitialize(unit: AudioUnit) -> OSStatus;
        pub fn AudioOutputUnitStart(unit: AudioUnit) -> OSStatus;
        pub fn AudioOutputUnitStop(unit: AudioUnit) -> OSStatus;

        pub fn AudioUnitRender(
            unit: AudioUnit,
            io_action_flags: *mut u32,
            in_time_stamp: *const AudioTimeStamp,
            in_output_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut AudioBufferList,
        ) -> OSStatus;

        // macOS 15+ AudioHardwareCreateProcessTap
        pub fn AudioHardwareCreateProcessTap(
            tap_description: *const c_void, // CATapDescription*
            out_tap_id: *mut AudioObjectID,
        ) -> OSStatus;

        pub fn AudioHardwareDestroyProcessTap(tap_id: AudioObjectID) -> OSStatus;
    }
}

/// Context passed to the audio render callback.
struct CallbackContext {
    producer: HeapProd<f32>,
    channels: u32,
}

/// The audio render callback. Called on the real-time audio thread.
/// MUST NOT allocate, lock, or block.
unsafe extern "C" fn render_callback(
    in_ref_con: *mut std::os::raw::c_void,
    _io_action_flags: *mut u32,
    _in_time_stamp: *const ffi::AudioTimeStamp,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut ffi::AudioBufferList,
) -> ffi::OSStatus {
    let ctx = &mut *(in_ref_con as *mut CallbackContext);

    if io_data.is_null() {
        return 0;
    }

    let buffer_list = &*io_data;
    if buffer_list.number_buffers == 0 {
        return 0;
    }

    // Read from first buffer (interleaved or mono)
    let buffer = &buffer_list.buffers[0];
    let num_samples = buffer.data_byte_size as usize / std::mem::size_of::<f32>();

    if !buffer.data.is_null() && num_samples > 0 {
        let samples = std::slice::from_raw_parts(buffer.data as *const f32, num_samples);

        // Downmix to mono if stereo
        if ctx.channels >= 2 {
            for chunk in samples.chunks(ctx.channels as usize) {
                let mono = chunk.iter().sum::<f32>() / ctx.channels as f32;
                let _ = ctx.producer.try_push(mono);
            }
        } else {
            for &sample in samples {
                let _ = ctx.producer.try_push(sample);
            }
        }
    }

    0
}

pub struct AudioTap {
    audio_unit: ffi::AudioUnit,
    tap_id: Option<ffi::AudioObjectID>,
    _callback_context: Box<CallbackContext>,
    config: AudioConfig,
}

// Safety: AudioUnit is accessed only from this struct's methods and the callback.
// The callback context is pinned in a Box and referenced by pointer.
unsafe impl Send for AudioTap {}

impl AudioTap {
    /// Create and start an audio tap on system audio output.
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        unsafe { Self::create_tap(producer, config) }
    }

    unsafe fn create_tap(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        // 1. Create CATapDescription via Objective-C runtime
        use objc2::runtime::{AnyClass, AnyObject};
        use objc2::msg_send;

        let tap_desc_class = AnyClass::get(c"CATapDescription")
            .ok_or_else(|| anyhow!(
                "CATapDescription class not found. Requires macOS 15+."
            ))?;

        // +[CATapDescription alloc] then -[CATapDescription initStereoGlobalTapButExcludeProcesses:]
        let tap_desc: *mut AnyObject = msg_send![tap_desc_class, alloc];
        // Create an empty NSArray for the exclusion list
        let nsarray_class = AnyClass::get(c"NSArray")
            .ok_or_else(|| anyhow!("NSArray class not found"))?;
        let empty_array: *mut AnyObject = msg_send![nsarray_class, array];
        let tap_desc: *mut AnyObject = msg_send![tap_desc, initStereoGlobalTapButExcludeProcesses: empty_array];

        if tap_desc.is_null() {
            return Err(anyhow!("Failed to create CATapDescription"));
        }

        // 2. Create the process tap
        let mut tap_id: ffi::AudioObjectID = 0;
        let status = ffi::AudioHardwareCreateProcessTap(
            tap_desc as *const _,
            &mut tap_id,
        );
        if status != 0 {
            return Err(anyhow!(
                "AudioHardwareCreateProcessTap failed with status {}. \
                 Make sure you've granted audio capture permissions.",
                status
            ));
        }

        // 3. Create AUHAL AudioUnit
        let desc = ffi::AudioComponentDescription {
            component_type: ffi::kAudioUnitType_Output,
            component_sub_type: ffi::kAudioUnitSubType_HALOutput,
            component_manufacturer: ffi::kAudioUnitManufacturer_Apple,
            component_flags: 0,
            component_flags_mask: 0,
        };

        let component = ffi::AudioComponentFindNext(std::ptr::null_mut(), &desc);
        if component.is_null() {
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("Could not find HAL output AudioComponent"));
        }

        let mut audio_unit: ffi::AudioUnit = std::ptr::null_mut();
        let status = ffi::AudioComponentInstanceNew(component, &mut audio_unit);
        if status != 0 {
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioComponentInstanceNew failed: {}", status));
        }

        // 4. Enable input on the AUHAL (bus 1) and disable output (bus 0)
        let enable: u32 = 1;
        let disable: u32 = 0;
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_EnableIO,
            ffi::kAudioUnitScope_Input,
            1, // input bus
            &enable as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_EnableIO,
            ffi::kAudioUnitScope_Output,
            0, // output bus
            &disable as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 5. Set the tap's aggregate device as the input device
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioOutputUnitProperty_CurrentDevice,
            ffi::kAudioUnitScope_Global,
            0,
            &tap_id as *const _ as *const _,
            std::mem::size_of::<ffi::AudioDeviceID>() as u32,
        );

        // 6. Set stream format to float32, interleaved
        let format = ffi::AudioStreamBasicDescription {
            sample_rate: config.sample_rate as f64,
            format_id: ffi::kAudioFormatLinearPCM,
            format_flags: ffi::kAudioFormatFlagIsFloat | ffi::kAudioFormatFlagIsPacked,
            bytes_per_packet: 4 * config.channels,
            frames_per_packet: 1,
            bytes_per_frame: 4 * config.channels,
            channels_per_frame: config.channels,
            bits_per_channel: 32,
            reserved: 0,
        };
        ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioUnitProperty_StreamFormat,
            ffi::kAudioUnitScope_Output, // output scope of input bus
            1,
            &format as *const _ as *const _,
            std::mem::size_of::<ffi::AudioStreamBasicDescription>() as u32,
        );

        // 7. Set render callback
        let callback_context = Box::new(CallbackContext {
            producer,
            channels: config.channels,
        });
        let callback_struct = ffi::AURenderCallbackStruct {
            input_proc: render_callback,
            input_proc_ref_con: &*callback_context as *const _ as *mut _,
        };
        let status = ffi::AudioUnitSetProperty(
            audio_unit,
            ffi::kAudioUnitProperty_SetRenderCallback,
            ffi::kAudioUnitScope_Output,
            1, // input bus
            &callback_struct as *const _ as *const _,
            std::mem::size_of::<ffi::AURenderCallbackStruct>() as u32,
        );
        if status != 0 {
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("Failed to set render callback: {}", status));
        }

        // 8. Initialize and start
        let status = ffi::AudioUnitInitialize(audio_unit);
        if status != 0 {
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioUnitInitialize failed: {}", status));
        }

        let status = ffi::AudioOutputUnitStart(audio_unit);
        if status != 0 {
            ffi::AudioUnitUninitialize(audio_unit);
            ffi::AudioComponentInstanceDispose(audio_unit);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            return Err(anyhow!("AudioOutputUnitStart failed: {}", status));
        }

        log::info!("Audio tap started (tap_id={}, sample_rate={})", tap_id, config.sample_rate);

        Ok(Self {
            audio_unit,
            tap_id: Some(tap_id),
            _callback_context: callback_context,
            config,
        })
    }

    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
}

impl Drop for AudioTap {
    fn drop(&mut self) {
        unsafe {
            ffi::AudioOutputUnitStop(self.audio_unit);
            ffi::AudioUnitUninitialize(self.audio_unit);
            ffi::AudioComponentInstanceDispose(self.audio_unit);
            if let Some(tap_id) = self.tap_id {
                ffi::AudioHardwareDestroyProcessTap(tap_id);
                log::info!("Audio tap destroyed (tap_id={})", tap_id);
            }
        }
    }
}
