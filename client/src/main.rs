mod client;

use crate::client::Client;
use anyhow::{Context, Result};
use goxlr_ipc::{DaemonRequest, DaemonResponse, DeviceType, GoXLRCommand, UsbProductInformation};
use goxlr_ipc::{DeviceStatus, Socket};
use tokio::net::UnixStream;

#[tokio::main]
async fn main() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/goxlr.socket")
        .await
        .context("Could not connect to the GoXLR daemon process")?;
    let address = stream
        .peer_addr()
        .context("Could not get the address of the GoXLR daemon process")?;
    let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(address, &mut stream);
    let mut client = Client::new(socket);

    client
        .send(GoXLRCommand::GetStatus)
        .await
        .context("Could not retrieve device status")?;

    print_device(client.device());

    Ok(())
}

fn print_device(device: &DeviceStatus) {
    println!(
        "Device type: {}",
        match device.device_type {
            DeviceType::Unknown => "Unknown",
            DeviceType::Full => "GoXLR (Full)",
            DeviceType::Mini => "GoXLR (Mini)",
        }
    );

    if let Some(usb) = &device.usb_device {
        print_usb_info(usb);
    }
}

fn print_usb_info(usb: &UsbProductInformation) {
    println!(
        "USB Device version: {}.{}.{}",
        usb.version.0, usb.version.1, usb.version.2
    );
    println!("USB Device manufacturer: {}", usb.manufacturer_name);
    println!("USB Device name: {}", usb.product_name);
    println!("USB Device is claimed by Daemon: {}", usb.is_claimed);
    println!(
        "USB Device has kernel driver attached: {}",
        usb.has_kernel_driver_attached
    );
    println!(
        "USB Address: bus {}, address {}",
        usb.bus_number, usb.address
    );
}
