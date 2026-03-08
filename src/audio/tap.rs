use super::AudioConfig;
use anyhow::{anyhow, Result};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

// Core Audio FFI
#[allow(non_camel_case_types, non_upper_case_globals, dead_code)]
mod ffi {
    use std::os::raw::c_void;

    pub type OSStatus = i32;
    pub type AudioObjectID = u32;
    pub type AudioDeviceID = AudioObjectID;
    pub type AudioDeviceIOProcID = *mut c_void;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;
    pub const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    pub const kAudioObjectPropertyElementMain: u32 = 0;

    pub const kAudioHardwarePropertyDefaultOutputDevice: u32 =
        u32::from_be_bytes(*b"dOut");
    pub const kAudioDevicePropertyDeviceUID: u32 = u32::from_be_bytes(*b"uid ");

    #[repr(C)]
    pub struct AudioObjectPropertyAddress {
        pub selector: u32,
        pub scope: u32,
        pub element: u32,
    }

    #[repr(C)]
    pub struct AudioBufferList {
        pub number_buffers: u32,
        pub buffers: [AudioBuffer; 1],
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub number_channels: u32,
        pub data_byte_size: u32,
        pub data: *mut c_void,
    }

    #[link(name = "AudioToolbox", kind = "framework")]
    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        pub fn AudioHardwareCreateProcessTap(
            tap_description: *const c_void,
            out_tap_id: *mut AudioObjectID,
        ) -> OSStatus;

        pub fn AudioHardwareDestroyProcessTap(tap_id: AudioObjectID) -> OSStatus;

        pub fn AudioHardwareCreateAggregateDevice(
            description: *const c_void,
            out_device_id: *mut AudioDeviceID,
        ) -> OSStatus;

        pub fn AudioHardwareDestroyAggregateDevice(device_id: AudioDeviceID) -> OSStatus;

        pub fn AudioObjectGetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> OSStatus;

        pub fn AudioDeviceCreateIOProcIDWithBlock(
            out_io_proc_id: *mut AudioDeviceIOProcID,
            device_id: AudioDeviceID,
            dispatch_queue: *mut c_void,
            io_block: *const c_void,
        ) -> OSStatus;

        pub fn AudioDeviceDestroyIOProcID(
            device: AudioDeviceID,
            io_proc_id: AudioDeviceIOProcID,
        ) -> OSStatus;

        pub fn AudioDeviceStart(
            device: AudioDeviceID,
            io_proc_id: AudioDeviceIOProcID,
        ) -> OSStatus;

        pub fn AudioDeviceStop(
            device: AudioDeviceID,
            io_proc_id: AudioDeviceIOProcID,
        ) -> OSStatus;
    }

    extern "C" {
        pub fn dispatch_queue_create(
            label: *const std::os::raw::c_char,
            attr: *const c_void,
        ) -> *mut c_void;

        pub fn dispatch_release(object: *mut c_void);

        // Block class — take address to get the class pointer
        pub static _NSConcreteStackBlock: c_void;
    }
}

// ── Raw Objective-C Block ABI ──

#[repr(C)]
struct BlockDescriptor {
    reserved: u64,
    size: u64,
}

#[repr(C)]
struct IOBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: unsafe extern "C" fn(
        block: *const IOBlock,
        in_now: *const c_void,
        in_input_data: *const c_void,
        in_input_time: *const c_void,
        out_output_data: *mut c_void,
        in_output_time: *const c_void,
    ) -> i32,
    descriptor: *const BlockDescriptor,
    ctx_ptr: *mut CallbackContext,
}

static BLOCK_DESCRIPTOR: BlockDescriptor = BlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<IOBlock>() as u64,
};

unsafe extern "C" fn block_invoke(
    block: *const IOBlock,
    _in_now: *const c_void,
    in_input_data: *const c_void,
    _in_input_time: *const c_void,
    _out_output_data: *mut c_void,
    _in_output_time: *const c_void,
) -> i32 {
    let ctx = &mut *(*block).ctx_ptr;
    process_audio_buffer(in_input_data, ctx);
    0
}

struct CallbackContext {
    producer: HeapProd<f32>,
    channels: u32,
}

unsafe fn process_audio_buffer(input_data: *const c_void, ctx: &mut CallbackContext) {
    static CALLBACK_COUNT: AtomicU32 = AtomicU32::new(0);

    if input_data.is_null() {
        return;
    }

    let buffer_list = &*(input_data as *const ffi::AudioBufferList);
    if buffer_list.number_buffers == 0 {
        return;
    }

    let buf = &buffer_list.buffers[0];
    if buf.data.is_null() || buf.data_byte_size == 0 {
        return;
    }

    let num_samples = buf.data_byte_size as usize / std::mem::size_of::<f32>();
    let samples = std::slice::from_raw_parts(buf.data as *const f32, num_samples);

    let count = CALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
    if count < 5 {
        let max_val = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        log::debug!(
            "IOProc #{}: buffers={}, ch={}, bytes={}, samples={}, max={:.6}",
            count, buffer_list.number_buffers, buf.number_channels,
            buf.data_byte_size, num_samples, max_val,
        );
    }

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

fn get_default_output_device() -> Result<ffi::AudioDeviceID> {
    let mut device_id: ffi::AudioDeviceID = 0;
    let mut data_size = std::mem::size_of::<ffi::AudioDeviceID>() as u32;
    let address = ffi::AudioObjectPropertyAddress {
        selector: ffi::kAudioHardwarePropertyDefaultOutputDevice,
        scope: ffi::kAudioObjectPropertyScopeGlobal,
        element: ffi::kAudioObjectPropertyElementMain,
    };
    let status = unsafe {
        ffi::AudioObjectGetPropertyData(
            ffi::kAudioObjectSystemObject, &address, 0, std::ptr::null(),
            &mut data_size, &mut device_id as *mut _ as *mut _,
        )
    };
    if status != 0 {
        return Err(anyhow!("Failed to get default output device: {}", status));
    }
    log::debug!("Default output device ID: {}", device_id);
    Ok(device_id)
}

fn get_device_uid(device_id: ffi::AudioDeviceID) -> Result<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    let address = ffi::AudioObjectPropertyAddress {
        selector: ffi::kAudioDevicePropertyDeviceUID,
        scope: ffi::kAudioObjectPropertyScopeGlobal,
        element: ffi::kAudioObjectPropertyElementMain,
    };
    let mut uid_ref: core_foundation::string::CFStringRef = std::ptr::null();
    let mut data_size = std::mem::size_of::<core_foundation::string::CFStringRef>() as u32;
    let status = unsafe {
        ffi::AudioObjectGetPropertyData(
            device_id, &address, 0, std::ptr::null(),
            &mut data_size, &mut uid_ref as *mut _ as *mut _,
        )
    };
    if status != 0 {
        return Err(anyhow!("Failed to get device UID for {}: {}", device_id, status));
    }
    let cf_string: CFString = unsafe { CFString::wrap_under_get_rule(uid_ref) };
    let uid = cf_string.to_string();
    log::debug!("Device {} UID: {}", device_id, uid);
    Ok(uid)
}

pub struct AudioTap {
    tap_id: ffi::AudioObjectID,
    aggregate_device_id: ffi::AudioDeviceID,
    io_proc_id: ffi::AudioDeviceIOProcID,
    dispatch_queue: *mut c_void,
    _block: Box<IOBlock>,
    _callback_context: *mut CallbackContext,
}

unsafe impl Send for AudioTap {}

impl AudioTap {
    pub fn new(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        unsafe { Self::create_tap(producer, config) }
    }

    unsafe fn create_tap(producer: HeapProd<f32>, config: AudioConfig) -> Result<Self> {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject};

        // ── Step 1: CATapDescription with UUID ──
        let tap_desc_class = AnyClass::get(c"CATapDescription")
            .ok_or_else(|| anyhow!("CATapDescription class not found. Requires macOS 15+."))?;

        let tap_desc: *mut AnyObject = msg_send![tap_desc_class, alloc];
        let nsarray_class = AnyClass::get(c"NSArray")
            .ok_or_else(|| anyhow!("NSArray class not found"))?;
        let empty_array: *mut AnyObject = msg_send![nsarray_class, array];
        let tap_desc: *mut AnyObject =
            msg_send![tap_desc, initStereoGlobalTapButExcludeProcesses: empty_array];
        if tap_desc.is_null() {
            return Err(anyhow!("Failed to create CATapDescription"));
        }

        let nsuuid_class = AnyClass::get(c"NSUUID")
            .ok_or_else(|| anyhow!("NSUUID class not found"))?;
        let tap_uuid: *mut AnyObject = msg_send![nsuuid_class, alloc];
        let tap_uuid: *mut AnyObject = msg_send![tap_uuid, init];
        let _: () = msg_send![tap_desc, setUUID: tap_uuid];
        let uuid_nsstring: *mut AnyObject = msg_send![tap_uuid, UUIDString];
        let _: () = msg_send![tap_desc, setMuteBehavior: 0i64];

        // ── Step 2: Process tap ──
        let mut tap_id: ffi::AudioObjectID = 0;
        let status = ffi::AudioHardwareCreateProcessTap(tap_desc as *const _, &mut tap_id);
        if status != 0 {
            return Err(anyhow!(
                "AudioHardwareCreateProcessTap failed: {}. Requires macOS 15+.", status
            ));
        }
        log::debug!("Created process tap with ID {}", tap_id);

        // ── Step 3: Output device UID ──
        let output_device_id = get_default_output_device()?;
        let output_uid = get_device_uid(output_device_id)?;

        // ── Step 4: Aggregate device ──
        let aggregate_uid = uuid::Uuid::new_v4().to_string();
        let aggregate_device_id =
            Self::create_aggregate_device(&output_uid, &aggregate_uid, uuid_nsstring)?;
        log::debug!("Created aggregate device with ID {}", aggregate_device_id);

        // ── Step 5: IOProc ──
        let ctx_ptr = Box::into_raw(Box::new(CallbackContext {
            producer,
            channels: config.channels,
        }));

        let io_block = Box::new(IOBlock {
            isa: &ffi::_NSConcreteStackBlock as *const c_void,
            flags: 0,
            reserved: 0,
            invoke: block_invoke,
            descriptor: &BLOCK_DESCRIPTOR,
            ctx_ptr,
        });

        let queue_label = c"com.terminal-vibes.audio";
        let dispatch_queue = ffi::dispatch_queue_create(queue_label.as_ptr(), std::ptr::null());

        let mut io_proc_id: ffi::AudioDeviceIOProcID = std::ptr::null_mut();
        let status = ffi::AudioDeviceCreateIOProcIDWithBlock(
            &mut io_proc_id,
            aggregate_device_id,
            dispatch_queue,
            &*io_block as *const IOBlock as *const c_void,
        );
        if status != 0 {
            let _ = Box::from_raw(ctx_ptr);
            ffi::AudioHardwareDestroyAggregateDevice(aggregate_device_id);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            ffi::dispatch_release(dispatch_queue);
            return Err(anyhow!("AudioDeviceCreateIOProcIDWithBlock failed: {}", status));
        }
        log::debug!("Created IOProc block on aggregate device");

        let status = ffi::AudioDeviceStart(aggregate_device_id, io_proc_id);
        if status != 0 {
            let _ = Box::from_raw(ctx_ptr);
            ffi::AudioDeviceDestroyIOProcID(aggregate_device_id, io_proc_id);
            ffi::AudioHardwareDestroyAggregateDevice(aggregate_device_id);
            ffi::AudioHardwareDestroyProcessTap(tap_id);
            ffi::dispatch_release(dispatch_queue);
            return Err(anyhow!("AudioDeviceStart failed: {}", status));
        }

        log::info!(
            "Audio tap started (tap_id={}, aggregate_device={}, sample_rate={})",
            tap_id, aggregate_device_id, config.sample_rate
        );

        Ok(Self {
            tap_id,
            aggregate_device_id,
            io_proc_id,
            dispatch_queue,
            _block: io_block,
            _callback_context: ctx_ptr,
        })
    }

    unsafe fn create_aggregate_device(
        output_uid: &str,
        aggregate_uid: &str,
        tap_uuid_nsstring: *mut objc2::runtime::AnyObject,
    ) -> Result<ffi::AudioDeviceID> {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;

        let v_output_uid = CFString::new(output_uid);

        let sub_device_dict = CFDictionary::from_CFType_pairs(&[(
            CFString::from_static_string("uid").as_CFType(),
            v_output_uid.as_CFType(),
        )]);
        let sub_device_array =
            core_foundation::array::CFArray::from_CFTypes(&[sub_device_dict.as_CFType()]);

        let uuid_cf = CFString::wrap_under_get_rule(
            tap_uuid_nsstring as core_foundation::string::CFStringRef,
        );

        let tap_dict = CFDictionary::from_CFType_pairs(&[
            (CFString::from_static_string("uid").as_CFType(), uuid_cf.as_CFType()),
            (CFString::from_static_string("drift").as_CFType(), CFBoolean::true_value().as_CFType()),
        ]);
        let tap_array =
            core_foundation::array::CFArray::from_CFTypes(&[tap_dict.as_CFType()]);

        let description = CFDictionary::from_CFType_pairs(&[
            (CFString::from_static_string("name").as_CFType(), CFString::new("terminal-vibes-tap").as_CFType()),
            (CFString::from_static_string("uid").as_CFType(), CFString::new(aggregate_uid).as_CFType()),
            (CFString::from_static_string("master").as_CFType(), v_output_uid.as_CFType()),
            (CFString::from_static_string("private").as_CFType(), CFBoolean::true_value().as_CFType()),
            (CFString::from_static_string("stacked").as_CFType(), CFBoolean::false_value().as_CFType()),
            (CFString::from_static_string("tapautostart").as_CFType(), CFBoolean::true_value().as_CFType()),
            (CFString::from_static_string("subdevices").as_CFType(), sub_device_array.as_CFType()),
            (CFString::from_static_string("taps").as_CFType(), tap_array.as_CFType()),
        ]);

        let mut aggregate_device_id: ffi::AudioDeviceID = 0;
        let status = ffi::AudioHardwareCreateAggregateDevice(
            description.as_concrete_TypeRef() as *const _,
            &mut aggregate_device_id,
        );
        if status != 0 {
            return Err(anyhow!("AudioHardwareCreateAggregateDevice failed: {}", status));
        }
        Ok(aggregate_device_id)
    }

}

impl Drop for AudioTap {
    fn drop(&mut self) {
        unsafe {
            let _ = ffi::AudioDeviceStop(self.aggregate_device_id, self.io_proc_id);
            let _ = ffi::AudioDeviceDestroyIOProcID(self.aggregate_device_id, self.io_proc_id);
            let _ = ffi::AudioHardwareDestroyAggregateDevice(self.aggregate_device_id);
            ffi::AudioHardwareDestroyProcessTap(self.tap_id);
            ffi::dispatch_release(self.dispatch_queue);
            let _ = Box::from_raw(self._callback_context);
            log::info!(
                "Audio tap destroyed (tap_id={}, aggregate_device={})",
                self.tap_id, self.aggregate_device_id
            );
        }
    }
}
