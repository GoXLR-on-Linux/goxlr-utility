use std::os::raw::c_void;
use std::ptr::null;
use std::{mem, ptr};

use crate::platform::macos::device::StereoChannels;
use anyhow::bail;
use anyhow::Result;
use core_foundation::array::{
    kCFTypeArrayCallBacks, CFArrayAppendValue, CFArrayCreateMutable, CFMutableArrayRef,
};
use core_foundation::base::{kCFAllocatorDefault, TCFType, ToVoid, UInt32};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use coreaudio_sys::{
    kAudioAggregateDevicePropertyFullSubDeviceList, kAudioDevicePropertyDeviceUID,
    kAudioDevicePropertyPreferredChannelsForStereo, kAudioDevicePropertyTransportType,
    kAudioDeviceTransportTypeUSB, kAudioHardwareNoError, kAudioHardwarePropertyDevices,
    kAudioHardwarePropertyPlugInForBundleID, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyElementMaster, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
    kAudioObjectUnknown, kAudioPlugInCreateAggregateDevice, kAudioPlugInDestroyAggregateDevice,
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectSetPropertyData, AudioValueTranslation, OSStatus,
};
use log::debug;

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

/// Enumerates CoreAudio devices on macOS and returns only physical TC‑Helicon GoXLR devices.
///
/// Behavior:
/// - Queries the HAL for all `AudioDeviceID`s, reads each device's display name and UID,
///   then filters via `is_physical_goxlr_usb`.
/// - Filtering requires USB transport, excludes aggregates (UID containing `::`), and
///   matches the Apple UID scheme (`AppleUSBAudioEngine:` with vendor `TC‑Helicon` and product `GoXLR`).
/// - For each match, returns a `CoreAudioDevice` with `display_name` and `uid`.
///
/// Returns:
/// - `Ok(Vec<CoreAudioDevice>)` containing all matching physical GoXLR devices.
///
/// Errors:
/// - Fails only if the global device enumeration (`AudioObjectGetPropertyDataSize`/`AudioObjectGetPropertyData`)
///   for the device list fails. Per‑device property read failures are ignored and those devices are skipped.
///
/// Notes:
/// - Emits debug logs for each detected GoXLR UID.
/// - Does not create or interact with aggregate devices; it merely discovers physical devices.

pub fn get_goxlr_devices() -> Result<Vec<CoreAudioDevice>> {
    let props = addr(
        kAudioHardwarePropertyDevices,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
    );
    let mut size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &props, 0, null(), &mut size)
    };
    hal_check(status, "HAL size")?;

    let count = (size as usize) / std::mem::size_of::<AudioDeviceID>();
    let mut ids: Vec<AudioDeviceID> = vec![kAudioObjectUnknown; count];
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &props,
            0,
            null(),
            &mut size,
            ids.as_mut_ptr() as *mut _,
        )
    };
    hal_check(status, "HAL data")?;
    let devices = ids
        .into_iter()
        .filter(|&id| id != kAudioObjectUnknown)
        .filter_map(|id| {
            let name = get_audio_property_data(
                addr(
                    kAudioObjectPropertyName,
                    kAudioObjectPropertyScopeGlobal,
                    kAudioObjectPropertyElementMain,
                ),
                id,
            )
            .ok()?;
            let uid = get_audio_property_data(
                addr(
                    kAudioDevicePropertyDeviceUID,
                    kAudioObjectPropertyScopeGlobal,
                    kAudioObjectPropertyElementMain,
                ),
                id,
            )
            .ok()?;

            let uid_str = uid.to_string();
            if !is_physical_goxlr_usb(id, &uid_str) {
                return None;
            }

            debug!("Found GoXLR Device UID: {}", uid_str);

            Some(CoreAudioDevice {
                display_name: name.to_string(),
                uid: uid_str,
            })
        })
        .collect();

    Ok(devices)
}

fn addr(selector: u32, scope: u32, element: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    }
}

unsafe fn get_audio_property_into<T>(
    id: AudioObjectID,
    addr: &AudioObjectPropertyAddress,
    out: &mut T,
) -> Result<()> {
    let mut size = std::mem::size_of::<T>() as u32;
    let status =
        AudioObjectGetPropertyData(id, addr, 0, null(), &mut size, out as *mut _ as *mut _);
    hal_check(status, "AudioObjectGetPropertyData")?;
    Ok(())
}

fn get_audio_property_data(
    addr: AudioObjectPropertyAddress,
    id: AudioObjectID,
) -> Result<CFString> {
    let mut ref_str: CFStringRef = null();
    unsafe {
        get_audio_property_into(id, &addr, &mut ref_str)?;
    }
    Ok(unsafe { CFString::wrap_under_create_rule(ref_str) })
}

fn get_audio_property_u32(addr: AudioObjectPropertyAddress, id: AudioObjectID) -> Result<u32> {
    let mut v: u32 = 0;
    unsafe {
        get_audio_property_into(id, &addr, &mut v)?;
    }
    Ok(v)
}

fn hal_check(status: OSStatus, ctx: &str) -> Result<()> {
    if status != kAudioHardwareNoError as i32 {
        bail!("CoreAudio Error: ({}): {}", ctx, status);
    }
    Ok(())
}

fn is_physical_goxlr_usb(id: AudioObjectID, uid: &str) -> bool {
    let transport = get_audio_property_u32(
        addr(
            kAudioDevicePropertyTransportType,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
        ),
        id,
    )
    .ok();
    if transport != Some(kAudioDeviceTransportTypeUSB as u32) {
        return false;
    }
    if uid.contains("::") {
        return false;
    }
    if !uid.starts_with("AppleUSBAudioEngine:") {
        return false;
    }
    let parts: Vec<&str> = uid.split(':').collect();
    parts.len() >= 4 && parts[1] == "TC-Helicon" && parts[2] == "GoXLR"
}
