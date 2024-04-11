use std::ffi::c_char;
use std::os::raw::c_void;
use std::ptr::null;
use std::{mem, ptr};

use crate::platform::macos::device::StereoChannels;
use anyhow::bail;
use anyhow::Result;
use core_foundation::array::{
    kCFTypeArrayCallBacks, CFArrayAppendValue, CFArrayCreateMutable, CFMutableArrayRef,
};
use core_foundation::base::{kCFAllocatorDefault, CFType, TCFType, ToVoid, UInt32};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFMutableDictionary, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use coreaudio_sys::{
    kAudioAggregateDevicePropertyFullSubDeviceList, kAudioDevicePropertyDeviceUID,
    kAudioDevicePropertyPreferredChannelsForStereo, kAudioHardwareNoError,
    kAudioHardwarePropertyDevices, kAudioHardwarePropertyPlugInForBundleID,
    kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
    kAudioObjectUnknown, kAudioPlugInCreateAggregateDevice, kAudioPlugInDestroyAggregateDevice,
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData, AudioValueTranslation, KERN_SUCCESS,
};
use goxlr_usb::{PID_GOXLR_FULL, PID_GOXLR_MINI, VID_GOXLR};
use io_kit_sys::types::io_iterator_t;
use io_kit_sys::{
    kIOMasterPortDefault, IOIteratorNext, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingServices, IOServiceMatching,
};

const CORE_AUDIO_UID: &str = "com.apple.audio.CoreAudio";
const AGGREGATE_PREFIX: &str = "GoXLR-Utility::Aggregate";
const LEGACY_PREFIX: &str = "com.adecorp.goxlr";

pub struct CoreAudioDevice {
    display_name: String,
    pub(crate) uid: String,
}

pub fn get_id_for_uid(uid: &str) -> anyhow::Result<AudioObjectID> {
    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyPlugInForBundleID,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let size = 0u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &properties,
            0,
            ptr::null(),
            &size as *const _ as *mut _,
        )
    };
    if status != kAudioHardwareNoError as i32 {
        bail!("Error Lookup up Bundle ID: {}", status);
    }

    // If our size is 0, something's gone terribly wrong :D
    assert_ne!(size, 0);

    let mut plugin_id = kAudioObjectUnknown;
    let plugin_ref = CFString::new(uid);

    let translation_value = AudioValueTranslation {
        mInputData: &plugin_ref as *const CFString as *mut c_void,
        mInputDataSize: mem::size_of::<CFString>() as u32,
        mOutputData: &mut plugin_id as *mut AudioObjectID as *mut c_void,
        mOutputDataSize: mem::size_of::<AudioObjectID>() as u32,
    };

    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &properties,
            0,
            ptr::null(),
            &size as *const _ as *mut _,
            &translation_value as *const _ as *mut _,
        )
    };

    if status != kAudioHardwareNoError as i32 {
        bail!("Error Fetching CoreAudio Plugin: {}", status);
    }
    Ok(plugin_id)
}

pub fn get_uid_for_id(id: AudioObjectID) -> anyhow::Result<String> {
    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDeviceUID,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let uid: CFStringRef = null();
    let size = mem::size_of::<CFStringRef>();

    let uid = unsafe {
        let status = AudioObjectGetPropertyData(
            id,
            &properties,
            0,
            null(),
            &size as *const _ as *mut _,
            &uid as *const _ as *mut _,
        );

        if status != kAudioHardwareNoError as i32 {
            bail!("Error Extracting UID for {}", id);
        }

        CFString::wrap_under_get_rule(uid)
    };

    Ok(uid.to_string())
}

pub fn create_aggregate_device(channel: String, device: &CoreAudioDevice) -> Result<AudioDeviceID> {
    let core_audio_id = get_id_for_uid(CORE_AUDIO_UID)?;

    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioPlugInCreateAggregateDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    // I should probably have a method for this..
    let mut size = 0u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            core_audio_id,
            &properties,
            0,
            ptr::null(),
            &size as *const _ as *mut _,
        )
    };
    if status != kAudioHardwareNoError as i32 {
        bail!("Create Aggregate Error Getting Size: {}", status);
    }

    // We'll use the UID of the physical device as part of the aggregate's UID
    let uid = format!(
        "{}::{}::{}",
        AGGREGATE_PREFIX,
        device.uid,
        channel.replace(' ', "")
    );

    // Create the Dictionary responsible for building the Aggregate Device..
    let name = format!("{} ({})", channel, device.display_name);
    let dictionary = CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("name").as_CFType(),
            CFString::new(&name).as_CFType(),
        ),
        (
            CFString::new("uid").as_CFType(),
            CFString::new(&uid).as_CFType(),
        ),
        (
            CFString::new("private").as_CFType(),
            CFBoolean::false_value().as_CFType(),
        ),
        (
            CFString::new("stacked").as_CFType(),
            CFBoolean::false_value().as_CFType(),
        ),
    ]);

    let device_id = kAudioObjectUnknown;
    let status = unsafe {
        AudioObjectGetPropertyData(
            core_audio_id,
            &properties,
            mem::size_of_val(&dictionary) as UInt32,
            &dictionary as *const _ as *const c_void,
            &mut size as *mut UInt32,
            &device_id as *const _ as *mut _,
        )
    };

    // Bad Property Size - 561211770
    // Illegal Operation - 1852797029

    if status != kAudioHardwareNoError as i32 {
        bail!("Create Aggregate - Unable to Create Device: {}", status);
    }

    if device_id == kAudioObjectUnknown {
        bail!("Create Aggregate - Device broke?")
    }

    Ok(device_id)
}

pub fn destroy_aggregate_device(aggregate: AudioDeviceID) -> Result<()> {
    let core_audio_id = get_id_for_uid(CORE_AUDIO_UID)?;

    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioPlugInDestroyAggregateDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    // I should probably have a method for this..
    let size = mem::size_of::<AudioDeviceID>();

    let status = unsafe {
        AudioObjectGetPropertyData(
            core_audio_id,
            &properties,
            0,
            null(),
            &size as *const _ as *mut _,
            &aggregate as *const _ as *mut _,
        )
    };

    if status != kAudioHardwareNoError as i32 {
        bail!("CoreAudio Error: {}", status);
    }

    Ok(())
}

/// Adds a Sub-device to to an aggregate devices, normally the physical GoXLR Device
pub fn add_sub_device(aggregate: AudioDeviceID, uid: String) -> anyhow::Result<()> {
    // Ok, we need to add a sub-device to our aggregate (this is usually our GoXLR)..
    unsafe {
        let sub_device = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        CFArrayAppendValue(sub_device, CFString::new(&uid).to_void());

        let properties = AudioObjectPropertyAddress {
            mSelector: kAudioAggregateDevicePropertyFullSubDeviceList,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let size = mem::size_of::<CFMutableArrayRef>();
        let status = AudioObjectSetPropertyData(
            aggregate,
            &properties,
            0,
            ptr::null(),
            size as UInt32,
            &sub_device as *const _ as *const c_void,
        );

        if status != kAudioHardwareNoError as i32 {
            bail!("Error Executing Add: {}", status);
        }
    }
    Ok(())
}

/// Set's the Aggregates 'active' channels, this is normally the stereo channels for
/// the virtual outputs / inputs
pub fn set_active_channels(
    id: AudioDeviceID,
    input: bool,
    channels: StereoChannels,
) -> anyhow::Result<()> {
    let scope = if input {
        kAudioObjectPropertyScopeInput
    } else {
        kAudioObjectPropertyScopeOutput
    };

    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyPreferredChannelsForStereo,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMaster,
    };

    unsafe {
        let value: [UInt32; 2] = [channels.left, channels.right];

        let size = mem::size_of::<UInt32>() * 2;
        let _ = AudioObjectSetPropertyData(
            id,
            &properties,
            0,
            ptr::null(),
            size as UInt32,
            &value as *const _ as *const c_void,
        );
    }

    Ok(())
}

pub fn find_all_existing_aggregates() -> Result<Vec<AudioDeviceID>> {
    // Ok, we need to ask CoreAudio for a list of devices via kAudioHardwarePropertyDevices, then
    // iterate them all, fetch their UIDs, then compare it against ours.
    let properties = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    // Get the response size so we can prepare for it..
    let size = 0u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &properties,
            0,
            null(),
            &size as *const _ as *mut _,
        )
    };
    if status != kAudioHardwareNoError as i32 {
        bail!("CoreAudio Error: {}", status);
    }

    // We know that this request returns a list of AudioDeviceIDs, so we can work out how
    // many devices we're going to get back..
    let count: usize = size as usize / mem::size_of::<AudioDeviceID>();
    let mut device_ids: Vec<AudioDeviceID> = vec![];
    device_ids.reserve_exact(count);
    let status = unsafe {
        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &properties,
            0,
            null(),
            &size as *const _ as *mut _,
            device_ids.as_mut_ptr() as *mut _,
        );
        device_ids.set_len(count);
        status
    };

    if status != kAudioHardwareNoError as i32 {
        bail!("CoreAudio Error: {}", status);
    }

    let mut device_list = Vec::new();
    for device in device_ids {
        if let Ok(uid) = get_uid_for_id(device) {
            if uid.starts_with(AGGREGATE_PREFIX) || uid.starts_with(LEGACY_PREFIX) {
                device_list.push(device);
            }
        }
    }

    Ok(device_list)
}

/*
    This function iterates over all the present CoreAudio devices, and attempts to match
    their VID/PID to a physical GoXLR device. If found, returns the device's UID and it's
    display name according to MacOS.
*/
pub fn get_goxlr_devices() -> Result<Vec<CoreAudioDevice>> {
    let mut devices: Vec<CoreAudioDevice> = Vec::new();

    let mut iterator = mem::MaybeUninit::<io_iterator_t>::uninit();
    let matcher = unsafe { IOServiceMatching(b"IOAudioEngine\0".as_ptr() as *const c_char) };
    let status = unsafe {
        IOServiceGetMatchingServices(kIOMasterPortDefault, matcher, iterator.as_mut_ptr())
    };

    if status != KERN_SUCCESS as i32 {
        bail!("Failed to Get Matching Service: {}", status);
    }

    let vid = CFString::new("idVendor");
    let pid = CFString::new("idProduct");
    let uid = CFString::new("IOAudioEngineGlobalUniqueID");
    let dsc = CFString::new("IOAudioEngineDescription");

    loop {
        let service = unsafe { IOIteratorNext(iterator.assume_init()) };
        if service == 0 {
            break;
        }

        // Pull the properties for this device..
        let mut dictionary = mem::MaybeUninit::<CFMutableDictionaryRef>::uninit();
        unsafe {
            IORegistryEntryCreateCFProperties(
                service,
                dictionary.as_mut_ptr(),
                kCFAllocatorDefault,
                0,
            );
        }
        let properties: CFDictionary<CFString, CFType> = unsafe {
            CFMutableDictionary::wrap_under_get_rule(dictionary.assume_init()).to_immutable()
        };

        // Check to see if this result includes 'idVendor' and 'idProduct'..
        if properties.contains_key(&pid) && properties.contains_key(&vid) {
            // Pull out the values..
            let vid = properties.get(&vid).downcast::<CFNumber>().unwrap();
            let pid = properties.get(&pid).downcast::<CFNumber>().unwrap();

            // Check whether the Vendor is TC-Helicon..
            if vid.to_i32().unwrap() != VID_GOXLR as i32 {
                continue;
            }

            let pid = pid.to_i32().unwrap();
            // Check whether we're a GoXLR
            if pid == PID_GOXLR_FULL as i32 || pid == PID_GOXLR_MINI as i32 {
                // Get the UID of this device..
                if properties.contains_key(&uid) {
                    let uid = properties.get(&uid).downcast::<CFString>().unwrap();

                    if properties.contains_key(&dsc) {
                        let description = properties.get(&dsc).downcast::<CFString>().unwrap();
                        devices.push(CoreAudioDevice {
                            display_name: description.to_string(),
                            uid: uid.to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(devices)
}
