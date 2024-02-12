use std::ffi::c_void;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cocoa::appkit::{
    NSApplication, NSApplicationActivateIgnoringOtherApps, NSApplicationActivationPolicyAccessory,
    NSButton, NSEventModifierFlags, NSEventSubtype, NSEventType, NSImage, NSMenu, NSMenuItem,
    NSRunningApplication, NSStatusBar, NSStatusItem,
};
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSData};
use cocoa_foundation::base::id;
use cocoa_foundation::foundation::{NSPoint, NSSize, NSString, NSTimeInterval};
use enum_map::Enum;
use log::{debug, warn};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, YES};
use objc::{class, msg_send, sel, sel_impl};
use strum::{Display, EnumIter, IntoEnumIterator};
use tokio::select;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use goxlr_ipc::PathTypes;

use crate::events::EventTriggers::Open;
use crate::events::{DaemonState, EventTriggers};
use crate::tray::macos::TrayOption::{
    Configure, OpenPathIcons, OpenPathLogs, OpenPathMicProfiles, OpenPathPresets, OpenPathProfiles,
    OpenPathSamples, Quit,
};
use crate::ICON;

// MacOS is similar to Windows, except it expects the App loop to exist on the main thread..
pub fn handle_tray(state: DaemonState, tx: Sender<EventTriggers>) -> anyhow::Result<()> {
    // Eventually, we're going to need to spawn a new thread which can cause a shutdown from cocoa,
    // but until then.. eh..
    let show_tray = state.show_tray.clone();

    let (tray_tx, tray_rx) = channel(10);
    tokio::spawn(run_tray(tray_rx, state, tx));

    debug!("Starting MacOS Tray Runtime..");
    App::create(tray_tx, show_tray);
    debug!("MacOS Tray Runtime Stopped..");

    Ok(())
}

async fn run_tray(mut rx: Receiver<TrayOption>, mut state: DaemonState, tx: Sender<EventTriggers>) {
    loop {
        select! {
            Some(tray) = rx.recv() => {
                debug!("Received Tray Message! {:?}", tray);
                let _ = match tray {
                    Configure => tx.try_send(EventTriggers::Activate),
                    OpenPathProfiles => tx.try_send(Open(PathTypes::Profiles)),
                    OpenPathMicProfiles => tx.try_send(Open(PathTypes::MicProfiles)),
                    OpenPathPresets => tx.try_send(Open(PathTypes::Presets)),
                    OpenPathSamples => tx.try_send(Open(PathTypes::Samples)),
                    OpenPathIcons => tx.try_send(Open(PathTypes::Icons)),
                    OpenPathLogs => tx.try_send(Open(PathTypes::Logs)),
                    Quit => tx.try_send(EventTriggers::Stop)
                };
            },
            () = state.shutdown.recv() => {
               debug!("Shutting Down, Attempting to kill the NSApp..");
                unsafe {
                    stop_ns_application();
                    break;
                }
            }
        }
    }
}

unsafe fn stop_ns_application() {
    // First let the NSApplication know it's time to Stop..
    let app = Class::get("NSApplication").unwrap();
    let app: *mut Object = msg_send![app, sharedApplication];
    app.stop_(nil);

    // Next, we generate an Application Event..
    let event: *mut Object = msg_send![class!(NSEvent),
        otherEventWithType:NSEventType::NSApplicationDefined
        location:NSPoint::new(0., 0.)
        modifierFlags:NSEventModifierFlags::empty()
        timestamp:NSTimeInterval::default()
        windowNumber:0
        context:nil
        subtype:NSEventSubtype::NSWindowExposedEventType
        data1:0
        data2:0
    ];

    // Then we send it to the NSApplication. The application RunLoop only stops after the 'next'
    // event, so we'll force one to ensure shutdown.
    let () = msg_send![app, postEvent:event atStart:true];
}

#[derive(Display, Debug, Enum, EnumIter, Eq, PartialEq)]
enum TrayOption {
    Configure,
    OpenPathProfiles,
    OpenPathMicProfiles,
    OpenPathPresets,
    OpenPathSamples,
    OpenPathIcons,
    OpenPathLogs,
    Quit,
}

struct App {}

impl App {
    pub fn create(sender: Sender<TrayOption>, show_tray: Arc<AtomicBool>) {
        debug!("Preparing Tray..");

        // Step 1, create the initial release pool, and base menu..
        unsafe { NSAutoreleasePool::new(nil) };
        let menu = unsafe { NSMenu::new(nil).autorelease() };

        // Configure the Application..
        unsafe {
            let current = NSRunningApplication::currentApplication(nil);
            current.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
        };

        let app = unsafe {
            // Create a 'Shared' application..
            let app = Class::get("NSApplication").unwrap();
            let app: *mut Object = msg_send![app, sharedApplication];
            app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
            app.activateIgnoringOtherApps_(YES);
            app
        };

        let status = if show_tray.load(Ordering::Relaxed) {
            debug!("Spawning Tray..");
            unsafe {
                let status = NSStatusBar::systemStatusBar(nil)
                    .statusItemWithLength_(-1.)
                    .autorelease();

                let button = status.button();
                let icon = ICON;

                let nsdata = NSData::dataWithBytes_length_(
                    nil,
                    icon.as_ptr() as *const std::os::raw::c_void,
                    icon.len() as u64,
                );

                let nsimage = NSImage::initWithData_(NSImage::alloc(nil), nsdata);
                let new_size = NSSize::new(18.0, 18.0);

                button.setImage_(nsimage);
                let () = msg_send![nsimage, setSize: new_size];
                let () = msg_send![button, setImagePosition: 2];
                let () = msg_send![nsimage, setTemplate: false];

                status
            }
        } else {
            nil
        };

        // Ok, lets add some items to the tray :)
        debug!("Building Menu..");
        let sub_title = unsafe { NSString::alloc(nil).init_str("Open Path") };

        // Create the Main Tray Labels..
        let configure = App::get_label("Configure GoXLR", Configure, sender.clone());
        let quit = App::get_label("Quit", Quit, sender.clone());

        // Create SubMenu Items..
        let profiles = App::get_label("Profiles", OpenPathProfiles, sender.clone());
        let mic_profiles = App::get_label("Mic Profiles", OpenPathMicProfiles, sender.clone());
        let presets = App::get_label("Presets", OpenPathPresets, sender.clone());
        let samples = App::get_label("Samples", OpenPathSamples, sender.clone());
        let icons = App::get_label("Icons", OpenPathIcons, sender.clone());
        let logs = App::get_label("Logs", OpenPathLogs, sender.clone());

        debug!("Generating Sub Menu...");
        let sub_menu = unsafe {
            let menu_item = NSMenuItem::alloc(nil);
            let menu = NSMenu::new(nil).autorelease();

            let () = msg_send![menu, setTitle: sub_title];
            let () = msg_send![menu_item, setTitle: sub_title];
            let () = msg_send![menu_item, setSubmenu: menu];

            menu.addItem_(profiles);
            menu.addItem_(mic_profiles);
            menu.addItem_(App::get_separator());
            menu.addItem_(presets);
            menu.addItem_(samples);
            menu.addItem_(icons);
            menu.addItem_(App::get_separator());
            menu.addItem_(logs);

            menu_item
        };
        unsafe {
            // Create the Tray Labels..
            debug!("Generating Main Menu..");
            menu.addItem_(configure);
            menu.addItem_(App::get_separator());
            menu.addItem_(sub_menu);
            menu.addItem_(App::get_separator());
            menu.addItem_(quit);
        }

        unsafe {
            status.setMenu_(menu);

            app.run();
        }
    }

    fn get_label(label: &str, option: TrayOption, sender: Sender<TrayOption>) -> id {
        unsafe {
            let title = NSString::alloc(nil).init_str(label).autorelease();
            let no_key = NSString::alloc(nil).init_str("").autorelease();
            let action = sel!(action:);

            let item: *const Object = msg_send![App::make_menu_item_class(), alloc];
            let () = msg_send![item, initWithTitle:title action:action keyEquivalent:no_key];
            let () = msg_send![item, setTarget: item];

            let item = item as id;
            (*item).set_ivar("CALLBACK", option as usize);

            // Box up and add the sender..
            let boxed = Box::new(sender);
            let ptr = Box::into_raw(boxed);
            let ptr = ptr as *mut c_void as usize;
            (*item).set_ivar("SENDER", ptr);

            item
        }
    }

    fn get_separator() -> id {
        unsafe {
            let separator = NSMenuItem::separatorItem(nil);
            let () = msg_send![separator, retain];
            separator
        }
    }

    fn make_menu_item_class() -> &'static Class {
        let class_name = "TrayHandler";
        Class::get(class_name).unwrap_or_else(|| {
            debug!("Creating MenuHandler..");
            let superclass = class!(NSMenuItem);
            let mut decl = ClassDecl::new(class_name, superclass).unwrap();

            extern "C" fn handle(this: &Object, _: Sel, _: id) {
                let option: usize = unsafe { *this.get_ivar("CALLBACK") };
                let option = TrayOption::iter().nth(option).unwrap();

                // The sender should be boxed..
                let sender: Box<Sender<TrayOption>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<TrayOption>;
                    Box::from_raw(pointer)
                };

                // If this fails, we're out of luck really..
                if sender.try_send(option).is_err() {
                    warn!("Failed to send Tray Signal");
                }
                mem::forget(sender);
            }

            unsafe {
                decl.add_method(sel!(action:), handle as extern "C" fn(&Object, _, _));
                decl.add_ivar::<usize>("CALLBACK");
                decl.add_ivar::<usize>("SENDER");
            }

            decl.register()
        })
    }
}
