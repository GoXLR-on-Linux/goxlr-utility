#[derive(thiserror::Error, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConnectError {
    #[error("GoXLR not found")]
    DeviceNotFound,

    #[error("usb error")]
    UsbError(#[from] rusb::Error),

    #[error("device is not a GoXLR")]
    DeviceNotGoXLR,
}
