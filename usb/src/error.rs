#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("No GoXLR device was found")]
    DeviceNotFound,

    #[cfg(not(target_os = "windows"))]
    #[error("USB error: {0}")]
    UsbError(#[from] rusb::Error),

    #[error("Device is not a GoXLR")]
    DeviceNotGoXLR,

    #[error("Unable to Claim Interface")]
    DeviceNotClaimed,
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[cfg(not(target_os = "windows"))]
    #[error("USB error: {0}")]
    UsbError(#[from] rusb::Error),

    #[error("Malformed response from GoXLR")]
    MalformedResponse(#[from] std::io::Error),
}
