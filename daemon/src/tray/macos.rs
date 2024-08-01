use std::ffi::c_void;
use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

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
use tokio::sync::oneshot;

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
    tokio::spawn(run_tray(RunParams {
        tray_receiver: tray_rx,
        event_sender: tx.clone(),
        state: state.clone(),
    }));

    debug!("Starting MacOS Tray Runtime..");
    App::create(AppParams {
        sender: tray_tx,
        show_tray,
        state,
        global_tx: tx.clone(),
    });
    debug!("MacOS Tray Runtime Stopped..");

    Ok(())
}

struct RunParams {
    tray_receiver: Receiver<TrayOption>,
    event_sender: Sender<EventTriggers>,
    state: DaemonState,
}

async fn run_tray(mut p: RunParams) {
    loop {
        select! {
            Some(tray) = p.tray_receiver.recv() => {
                debug!("Received Tray Message! {:?}", tray);

                let tx = p.event_sender.clone();
                let _ = match tray {
                    Configure => tx.try_send(EventTriggers::Activate),
                    OpenPathProfiles => tx.try_send(Open(PathTypes::Profiles)),
                    OpenPathMicProfiles => tx.try_send(Open(PathTypes::MicProfiles)),
                    OpenPathPresets => tx.try_send(Open(PathTypes::Presets)),
                    OpenPathSamples => tx.try_send(Open(PathTypes::Samples)),
                    OpenPathIcons => tx.try_send(Open(PathTypes::Icons)),
                    OpenPathLogs => tx.try_send(Open(PathTypes::Logs)),
                    Quit => tx.try_send(EventTriggers::Stop(false))
                };
            },
            () = p.state.shutdown.recv() => {
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

struct AppParams {
    sender: Sender<TrayOption>,
    show_tray: Arc<AtomicBool>,
    state: DaemonState,
    global_tx: Sender<EventTriggers>,
}

impl App {
    pub fn create(p: AppParams) {
        debug!("Preparing Tray..");

        // Step 1, create the initial release pool, and base menu..
        unsafe { NSAutoreleasePool::new(nil) };

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

        let status = if p.show_tray.load(Ordering::Relaxed) {
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

                Some(status)
            }
        } else {
            None
        };

        // Ok, lets add some items to the tray :)
        if let Some(status) = status {
            debug!("Building Menu..");

            let menu = unsafe { NSMenu::new(nil).autorelease() };
            let sub_title = unsafe { NSString::alloc(nil).init_str("Open Path") };

            // Create the Main Tray Labels..
            let configure = App::get_label("Configure GoXLR", Configure, p.sender.clone());
            let quit = App::get_label("Quit", Quit, p.sender.clone());

            // Create SubMenu Items..
            let profiles = App::get_label("Profiles", OpenPathProfiles, p.sender.clone());
            let mic_profiles =
                App::get_label("Mic Profiles", OpenPathMicProfiles, p.sender.clone());
            let presets = App::get_label("Presets", OpenPathPresets, p.sender.clone());
            let samples = App::get_label("Samples", OpenPathSamples, p.sender.clone());
            let icons = App::get_label("Icons", OpenPathIcons, p.sender.clone());
            let logs = App::get_label("Logs", OpenPathLogs, p.sender.clone());

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
            }
        }

        unsafe {
            // Before we run, register with the observer to see if shutdown is going to happen..
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let notification_center: id = msg_send![workspace, notificationCenter];

            // Get the Distributed Notification Center (for Lock / Unlock Notifications)
            let dnc: id = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];

            debug!("Creating Controller..");
            let controller: id = msg_send![App::make_shutdown_hook_class(), alloc];

            debug!("Initialising..");
            let () = msg_send![controller, init];

            debug!("Boxing Sender..");
            let boxed = Box::new(p.global_tx.clone());
            let ptr = Box::into_raw(boxed);
            let ptr = ptr as *mut c_void as usize;
            (*controller).set_ivar("EVENT_SENDER", ptr);

            debug!("Boxing AtomicBool..");
            let boxed = Box::new(p.state.shutdown_blocking.clone());
            let ptr = Box::into_raw(boxed);
            let ptr = ptr as *mut c_void as usize;
            (*controller).set_ivar("STOP_BOOL", ptr);

            debug!("Registering Event..");
            let event = "NSWorkspaceWillPowerOffNotification";
            let event = NSString::alloc(nil).init_str(event).autorelease();

            debug!("Registering Class..");
            let () = msg_send![notification_center, addObserver:controller selector:sel!(computerWillShutDownNotification:) name:event object: nil];

            // We probably shouldn't share pointers to the senders, but seeing as MacOS locks
            // the entire NS runtime into a single thread, we should be safe here.
            let event = "NSWorkspaceWillSleepNotification";
            let event = NSString::alloc(nil).init_str(event).autorelease();
            let () = msg_send![notification_center, addObserver:controller selector:sel!(computerWillSleepNotification:) name: event object: nil];

            let event = "NSWorkspaceDidWakeNotification";
            let event = NSString::alloc(nil).init_str(event).autorelease();
            let () = msg_send![notification_center, addObserver:controller selector:sel!(computerWillWakeNotification:) name: event object: nil];

            let event = "com.apple.screenIsLocked";
            let event = NSString::alloc(nil).init_str(event).autorelease();
            let () = msg_send![dnc, addObserver:controller selector:sel!(screenIsLocked:) name: event object: nil];

            let event = "com.apple.screenIsUnlocked";
            let event = NSString::alloc(nil).init_str(event).autorelease();
            let () = msg_send![dnc, addObserver:controller selector:sel!(screenIsUnlocked:) name: event object: nil];

            debug!("Running..");
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

    fn make_shutdown_hook_class() -> &'static Class {
        let class_name = "PowerHandler";
        Class::get(class_name).unwrap_or_else(|| {
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new(class_name, superclass).unwrap();

            extern "C" fn handle_shutdown(this: &Object, _: Sel, notification: *const Object) {
                debug!("Received Shutdown Notification! {:?}", notification);
                let sender: Box<Sender<EventTriggers>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("EVENT_SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<EventTriggers>;
                    Box::from_raw(pointer)
                };

                let stop: Box<Arc<AtomicBool>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("STOP_BOOL");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Arc<AtomicBool>;
                    Box::from_raw(pointer)
                };

                // This is pretty similar to Windows, we loop until we're ready to die..
                let _ = sender.try_send(EventTriggers::Stop(false));

                // Now wait for the daemon to actually stop..
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    } else {
                        debug!("Waiting..");
                        sleep(Duration::from_millis(100));
                    }
                }

                mem::forget(sender);
                mem::forget(stop);
            }

            extern "C" fn handle_sleep(this: &Object, _: Sel, notification: *const Object) {
                debug!("Received Sleep Notification! {:?}", notification);
                let sender: Box<Sender<EventTriggers>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("EVENT_SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<EventTriggers>;
                    Box::from_raw(pointer)
                };

                // Pretty much copypasta from Windows which behaves in a similar way..
                let (tx, mut rx) = oneshot::channel();

                // Give a maximum of 1 second for a response..
                let milli_wait = 5;
                let max_wait = 1000 / milli_wait;
                let mut count = 0;

                if sender.try_send(EventTriggers::Sleep(tx)).is_ok() {
                    debug!("Awaiting Sleep Response..");
                    while rx.try_recv().is_err() {
                        sleep(Duration::from_millis(milli_wait));
                        count += 1;
                        if count > max_wait {
                            debug!("Timeout Exceeded, bailing.");
                            break;
                        }
                    }
                    debug!("Task Completed, allowing MacOS to Sleep");
                }
                mem::forget(sender);
            }
            extern "C" fn handle_wake(this: &Object, _: Sel, notification: *const Object) {
                debug!("Received Wake Notification! {:?}", notification);
                let sender: Box<Sender<EventTriggers>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("EVENT_SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<EventTriggers>;
                    Box::from_raw(pointer)
                };

                let (tx, _rx) = oneshot::channel();
                let _ = sender.try_send(EventTriggers::Wake(tx));

                mem::forget(sender);
            }

            extern "C" fn handle_lock(this: &Object, _: Sel, notification: *const Object) {
                debug!("Received Lock Notification.. {:?}", notification);

                let sender: Box<Sender<EventTriggers>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("EVENT_SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<EventTriggers>;
                    Box::from_raw(pointer)
                };

                let _ = sender.try_send(EventTriggers::Lock);
                mem::forget(sender);
            }

            extern "C" fn handle_unlock(this: &Object, _: Sel, notification: *const Object) {
                debug!("Received Unlock Notification.. {:?}", notification);
                let sender: Box<Sender<EventTriggers>> = unsafe {
                    let pointer_value: usize = *this.get_ivar("EVENT_SENDER");
                    let pointer = pointer_value as *mut c_void;
                    let pointer = pointer as *mut Sender<EventTriggers>;
                    Box::from_raw(pointer)
                };

                let _ = sender.try_send(EventTriggers::Unlock);
                mem::forget(sender);
            }

            unsafe {
                decl.add_method(
                    sel!(computerWillShutDownNotification:),
                    handle_shutdown as extern "C" fn(&Object, Sel, *const Object),
                );
                decl.add_method(
                    sel!(computerWillSleepNotification:),
                    handle_sleep as extern "C" fn(&Object, Sel, *const Object),
                );
                decl.add_method(
                    sel!(computerWillWakeNotification:),
                    handle_wake as extern "C" fn(&Object, Sel, *const Object),
                );
                decl.add_method(
                    sel!(screenIsLocked:),
                    handle_lock as extern "C" fn(&Object, Sel, *const Object),
                );
                decl.add_method(
                    sel!(screenIsUnlocked:),
                    handle_unlock as extern "C" fn(&Object, Sel, *const Object),
                );
                decl.add_ivar::<usize>("EVENT_SENDER");
                decl.add_ivar::<usize>("STOP_BOOL");
            }

            decl.register()
        })
    }
}
