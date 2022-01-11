#[derive(thiserror::Error, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConnectError {
    #[error("No GoXLR device was found")]
    DeviceNotFound,

    #[error("USB error: {0}")]
    UsbError(#[from] rusb::Error),

    #[error("Device is not a GoXLR")]
    DeviceNotGoXLR,
}
