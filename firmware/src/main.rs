use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use goxlr_ipc::{DeviceType, HardwareStatus, UsbProductInformation};
use goxlr_types::VersionNumber;
use goxlr_usb::device::base::{AttachGoXLR, FullGoXLRDevice};
use goxlr_usb::device::{find_devices, from_device};
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args[1] != "I-AM-REALLY-SURE" {
        eprintln!("Running this binary may damage your GoXLR, do not run it unless you know");
        eprintln!("*EXACTLY* what you're doing, this code is in development, it is *NOT* safe!");
        bail!("Aborting.");
    }

    let file = PathBuf::from(&args[2]);

    // This is pretty straight forward, Firstly find all the GoXLRs..
    let devices = find_devices();
    if devices.is_empty() {
        bail!("No GoXLR Devices Found!");
    }

    if devices.len() > 1 {
        bail!("More than one GoXLR Found (TODO: Serials.)");
    }

    // Create the Message Queues...
    let (disconnect_sender, mut disconnect_receiver) = mpsc::channel(32);
    let (event_sender, mut event_receiver) = mpsc::channel(32);

    let device = devices[0].clone();
    let device_clone = device.clone();
    let mut handled_device = from_device(device, disconnect_sender, event_sender)?;
    let descriptor = handled_device.get_descriptor()?;

    let device_type = match descriptor.product_id() {
        PID_GOXLR_FULL => DeviceType::Full,
        PID_GOXLR_MINI => DeviceType::Mini,
        _ => DeviceType::Unknown,
    };
    let device_version = descriptor.device_version();
    let version = (device_version.0, device_version.1, device_version.2);
    let usb_device = UsbProductInformation {
        manufacturer_name: descriptor.device_manufacturer(),
        product_name: descriptor.product_name(),
        bus_number: device_clone.bus_number(),
        address: device_clone.address(),
        identifier: device_clone.identifier().clone(),
        version,
    };
    let (mut serial_number, manufactured_date) = handled_device.get_serial_number()?;
    if serial_number.is_empty() {
        bail!("Unable to Obtain GoXLR Serial Number!");
    }

    handled_device.set_unique_identifier(serial_number.clone());

    let hardware = HardwareStatus {
        versions: handled_device.get_firmware_version()?,
        serial_number,
        manufactured_date,
        device_type,
        usb_device,
    };

    if hardware.device_type == DeviceType::Mini {
        bail!("This code has only been tested on the full device.");
    }

    let wait = sleep(Duration::from_secs(2));
    tokio::pin!(wait);

    // Now we're going to sit and wait for events..
    loop {
        tokio::select! {
            Some(serial) = event_receiver.recv() => {
                println!("Received Event from {}", serial);
            },
            Some(serial) = disconnect_receiver.recv() => {
                println!("Received Disconnect from {}", serial);
            }
            () = &mut wait => {
                // Trigger this again in about 136 years.. We'll do better later!
                wait.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(u32::MAX.into()));
                println!("Executing Firmware Update..");
                do_firmware_upload(&mut handled_device, &file).await?;
            }
        }
    }
}

async fn do_firmware_upload(device: &mut Box<dyn FullGoXLRDevice>, file: &PathBuf) -> Result<()> {
    let firmware = load_firmware_file(file)?;

    println!("Stopping Device Polling..");
    device.stop_polling();

    sleep(Duration::from_secs(2)).await;
    println!("Starting..");

    println!("Putting Device in Firmware Update Mode..");
    device.begin_firmware_upload()?;

    println!("Beginning Erasure of Update Partition..");
    device.begin_erase_nvr();

    let mut progress = 0;
    while progress != 0xff {
        sleep(Duration::from_millis(100)).await;
        progress = device.poll_erase_nvr()?;

        // Can output a percentage here..
    }
    println!("Complete.");
    println!("Beginning Sending of Data..");

    // This is to match the Official App
    let chunk_size = 1012;

    let mut sent = 0;
    for chunk in firmware.chunks(chunk_size) {
        device.send_firmware_packet(sent, chunk);
        sent += chunk.len() as u64;
    }

    println!("Data Sent, Beginning Validation..");
    let total_bytes = firmware.len() as u32;
    let mut remaining_bytes = sent as u32;

    // This should never fail, unless there's been a chunking issue.
    if total_bytes != remaining_bytes {
        bail!("Error with Data Send");
    }

    let mut processed: u32 = 0;
    let mut hash_in = 0;

    while remaining_bytes > 0 {
        let (hash, count) = device.validate_firmware_packet(processed, hash_in, remaining_bytes)?;

        processed += count;
        if processed > total_bytes {
            bail!("Validation Failed!");
        }

        remaining_bytes -= count;

        // I've attempted to determine how the CRC32 for this hash is calculated, and never found a correct answer, it might
        // be possible to validate it, but the official app doesn't do so, it just sends it with the next packet. To the best
        // of my understanding, the next step does a separate CRC check on the device itself, so we have to hope that's OK :)
        hash_in = hash;
    }
    println!("Validation complete!");

    // So the GoXLR will note if something's gone wrong, the official app ignores it, but we'll see if we can do a slightly
    // better job, and try to inform the user as well as abort the firmware update process, if not, we'll duplicate the official
    // app behaviour, and the below applies..

    // It should be noted, the GoXLR does appear to return errors when something goes wrong, but the official app doesn't seem
    // to care, or register them, and proceeds as if they didn't happen. The GoXLR itself will take care of this behaviour, but
    // we should be able to at least inform the user that something has probably gone wrong :p

    Ok(())
}

fn load_firmware_file(file: &PathBuf) -> Result<Vec<u8>> {
    let firmware = std::fs::read(file)?;

    println!("{:?}", get_firmware_name(&firmware[0..16]));
    println!("{:?}", get_firmware_version(&firmware[24..32]));

    // 25 + 26

    Ok(firmware)
}

fn get_firmware_name(src: &[u8]) -> String {
    let mut end_index = 0;
    for byte in src {
        if *byte == 0x00 {
            break;
        }
        end_index += 1;
    }
    return String::from_utf8_lossy(&src[0..end_index]).to_string();
}

fn get_firmware_version(src: &[u8]) -> Result<VersionNumber> {
    println!("{}", src.len());
    println!("{:x?}", src);

    // Unpack the firmware version..
    let mut cursor = Cursor::new(src);
    let firmware_packed = cursor.read_u32::<LittleEndian>()?;
    let firmware_build = cursor.read_u32::<LittleEndian>()?;
    let firmware = VersionNumber(
        firmware_packed >> 12,
        (firmware_packed >> 8) & 0xF,
        firmware_packed & 0xFF,
        firmware_build,
    );

    Ok(firmware)
}
