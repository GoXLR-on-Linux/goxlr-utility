// Coming soon, to a utility near you..
// MacOS AutoStart Support :p

use crate::ICON;
use cocoa::appkit::NSImage;
use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::{NSData, NSString};
use objc::{class, msg_send, sel, sel_impl};

pub fn display_error(message: String) {
    unsafe {
        let alert: id = msg_send![class!(NSAlert), alloc];
        let () = msg_send![alert, init];
        let () = msg_send![alert, autorelease];
        let () = msg_send![alert, setIcon: get_icon()];
        let () = msg_send![alert, setMessageText: NSString::alloc(nil).init_str("GoXLR Utility")];
        let () = msg_send![alert, setInformativeText: NSString::alloc(nil).init_str(&message)];
        let () = msg_send![alert, setAlertStyle: 2];

        // Get the Window..
        let window: id = msg_send![alert, window];
        let () = msg_send![window, setLevel: 10];

        // Send the Alert..
        let () = msg_send![alert, runModal];
    }
}

fn get_icon() -> id {
    unsafe {
        let data = NSData::dataWithBytes_length_(
            nil,
            ICON.as_ptr() as *const std::os::raw::c_void,
            ICON.len() as u64,
        );
        NSImage::initWithData_(NSImage::alloc(nil), data)
    }
}
