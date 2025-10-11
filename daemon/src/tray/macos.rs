#![allow(deprecated)]

use dispatch2::Queue;
use enum_map::Enum;
use log::{debug, warn};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    define_class, msg_send, sel, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
    Message,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy,
    NSApplicationDelegate, NSCellImagePosition, NSEvent, NSEventModifierFlags, NSEventSubtype,
    NSEventType, NSImage, NSMenu, NSMenuItem, NSRunningApplication, NSStatusBar, NSWorkspace,
};
use objc2_foundation::{
    NSAutoreleasePool, NSData, NSDistributedNotificationCenter, NSNotification, NSNotificationName,
    NSPoint, NSSize, NSString, NSTimeInterval,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
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
use crate::ICON_MAC;

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
    let main_queue = Queue::main();
    main_queue.exec_async(|| {
        let mtm = MainThreadMarker::new().unwrap();
        let app = NSApplication::sharedApplication(mtm);
        app.stop(None);

        // Next, we generate an Application Event..
        let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::ApplicationDefined,
            NSPoint::new(0., 0.),
            NSEventModifierFlags::empty(),
            NSTimeInterval::default(),
            0,
            None,
            NSEventSubtype::WindowExposed.0,
            0,
            0,
        ).unwrap();

        // Then we send it to the NSApplication. The application RunLoop only stops after the 'next'
        // event, so we'll force one to ensure shutdown.
        app.postEvent_atStart(&event, true)
    });
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
        let mtm = MainThreadMarker::new().unwrap();

        // Step 1, create the initial release pool, and base menu..
        unsafe { NSAutoreleasePool::new() };

        // Configure the Application..
        let current = NSRunningApplication::currentApplication();
        current.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.activateIgnoringOtherApps(true);

        // Setting the App Delegate..
        let delegate = UtilityDelegate::new(
            mtm,
            p.sender.clone(),
            p.global_tx.clone(),
            p.state.shutdown_blocking.clone(),
        );
        let object = ProtocolObject::from_ref(&*delegate);
        app.setDelegate(Some(object));

        let status = if p.show_tray.load(Ordering::Relaxed) {
            debug!("Spawning Tray..");
            let status = NSStatusBar::systemStatusBar().statusItemWithLength(-1.);

            let button = status.button(mtm);
            let data = NSData::with_bytes(ICON_MAC);
            if let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) {
                icon.setSize(NSSize::new(18., 18.));
                icon.setTemplate(false);

                if let Some(button) = button {
                    button.setImage(Some(&*icon));
                    button.setImagePosition(NSCellImagePosition::ImageLeft)
                }
            }

            Some(status)
        } else {
            None
        };

        // Ok, lets add some items to the tray :)
        if let Some(status) = status {
            debug!("Building Menu..");

            let menu = NSMenu::new(mtm);
            let sub_title = NSString::from_str("Open Path");

            // Create the Main Tray Labels..
            let configure = App::get_label(mtm, "Configure GoXLR", Configure);
            let quit = App::get_label(mtm, "Quit", Quit);

            // Create SubMenu Items..
            let profiles = App::get_label(mtm, "Profiles", OpenPathProfiles);
            let mic_profiles = App::get_label(mtm, "Mic Profiles", OpenPathMicProfiles);
            let presets = App::get_label(mtm, "Presets", OpenPathPresets);
            let samples = App::get_label(mtm, "Samples", OpenPathSamples);
            let icons = App::get_label(mtm, "Icons", OpenPathIcons);
            let logs = App::get_label(mtm, "Logs", OpenPathLogs);

            debug!("Generating Sub Menu...");
            let sub_menu = {
                let menu_item = NSMenuItem::new(mtm);
                let menu = NSMenu::new(mtm);

                menu.setTitle(&sub_title);
                menu_item.setTitle(&sub_title);
                menu_item.setSubmenu(Some(&menu));

                menu.addItem(&profiles);
                menu.addItem(&mic_profiles);
                menu.addItem(&App::get_separator(mtm));
                menu.addItem(&presets);
                menu.addItem(&samples);
                menu.addItem(&icons);
                menu.addItem(&App::get_separator(mtm));
                menu.addItem(&logs);

                menu_item
            };

            // Create the Tray Labels
            debug!("Generating Main Menu..");
            menu.addItem(&configure);
            menu.addItem(&App::get_separator(mtm));
            menu.addItem(&sub_menu);
            menu.addItem(&App::get_separator(mtm));
            menu.addItem(&quit);

            unsafe {
                status.setMenu(Some(&*menu));
            }
        }

        // Before we run, register with the observer to see if shutdown is going to happen..
        let workspace = NSWorkspace::sharedWorkspace();
        let notification_center = workspace.notificationCenter();

        // Get the Distributed Notification Center (for Lock / Unlock Notifications)
        let dnc = NSDistributedNotificationCenter::defaultCenter();

        debug!("Registering Event..");
        let event = "NSWorkspaceWillPowerOffNotification";
        let event = NSNotificationName::from_str(event);

        debug!("Registering Class..");
        notification_center.addObserver_selector_name_object(
            &delegate,
            sel!(computerWillShutDownNotification:),
            Some(&event),
            None,
        );

        // We probably shouldn't share pointers to the senders, but seeing as MacOS locks
        // the entire NS runtime into a single thread, we should be safe here.
        let event = "NSWorkspaceWillSleepNotification";
        let event = NSNotificationName::from_str(event);
        notification_center.addObserver_selector_name_object(
            &delegate,
            sel!(computerWillSleepNotification:),
            Some(&event),
            None,
        );

        let event = "NSWorkspaceDidWakeNotification";
        let event = NSNotificationName::from_str(event);
        notification_center.addObserver_selector_name_object(
            &delegate,
            sel!(computerWillWakeNotification:),
            Some(&event),
            None,
        );

        let event = "com.apple.screenIsLocked";
        let event = NSNotificationName::from_str(event);
        dnc.addObserver_selector_name_object(&delegate, sel!(screenIsLocked:), Some(&event), None);

        let event = "com.apple.screenIsUnlocked";
        let event = NSNotificationName::from_str(event);
        dnc.addObserver_selector_name_object(
            &delegate,
            sel!(screenIsUnlocked:),
            Some(&event),
            None,
        );

        debug!("Running..");
        app.run();
    }

    fn get_label(mtm: MainThreadMarker, label: &str, option: TrayOption) -> Retained<NSMenuItem> {
        unsafe {
            let title = NSString::from_str(label);

            let item = NSMenuItem::new(mtm);
            item.setAction(Some(sel!(menu_item:)));
            item.setTitle(&title);

            // Two approaches for storing enum data in NSMenuItem:
            //
            // 1. Store enum variant index in tag (safer, simpler):
            //    - Convert enum to its index: option as isize
            //    - Retrieve with: TrayOption::iter().nth(item.tag() as usize).unwrap()
            //    - Limited to simple enums without associated data
            //
            // 2. Store pointer to boxed data in tag (more flexible but unsafe):
            //    let data = Box::new(option);
            //    let ptr = Box::into_raw(data);
            //    let tag_value = ptr as usize as isize;
            //    item.setTag(tag_value);
            //
            //    // Retrieve with:
            //    let ptr = item.tag() as usize as *mut TrayOption;
            //    let option = &*ptr;  // Note: must not take ownership to avoid double-free
            //
            //    // WARNING: This causes memory leaks as boxed data is never freed
            //    // Only use when necessary for complex data that can't be encoded as an index

            item.setTag(option as isize);

            item
        }
    }

    fn get_separator(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
        let separator = NSMenuItem::separatorItem(mtm);
        separator.retain();
        separator
    }
}

pub(crate) struct State {
    sender: Sender<TrayOption>,
    global_tx: Sender<EventTriggers>,
    shutdown_signal: Arc<AtomicBool>,
}

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "UtilityDelegate"]
    #[ivars = State]
    pub(crate) struct UtilityDelegate;

    unsafe impl NSObjectProtocol for UtilityDelegate {}

    unsafe impl NSApplicationDelegate for UtilityDelegate {
        //Showcase function for now
        #[unsafe(method(menu_item:))]
        unsafe fn menu_item(&self, item: &NSMenuItem) {
            if let Some(option) = TrayOption::iter().nth(item.tag() as usize) {
                if self.ivars().sender.try_send(option).is_err() {
                    warn!("Failed to send Tray Signal");
                }
            }
        }
    }

    impl UtilityDelegate {
        #[unsafe(method(computerWillShutDownNotification:))]
        unsafe fn computer_will_shutdown(&self, notification: &NSNotification) {
            debug!("Received Shutdown Notification! {:?}", notification);
                // This is pretty similar to Windows, we loop until we're ready to die..
                let _ = self.ivars().global_tx.try_send(EventTriggers::Stop(false));

                // Now wait for the daemon to actually stop..
                loop {
                    if self.ivars().shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    } else {
                        debug!("Waiting..");
                        sleep(Duration::from_millis(100));
                    }
                }
        }

        #[unsafe(method(computerWillSleepNotification:))]
        unsafe fn computer_will_sleep(&self, notification: &NSNotification) {
            debug!("Received Sleep Notification! {:?}", notification);
                // Pretty much copypasta from Windows which behaves in a similar way..
                let (tx, mut rx) = oneshot::channel();

                // Give a maximum of 1 second for a response..
                let milli_wait = 5;
                let max_wait = 1000 / milli_wait;
                let mut count = 0;

                if self.ivars().global_tx.try_send(EventTriggers::Sleep(tx)).is_ok() {
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
        }

        #[unsafe(method(computerWillWakeNotification:))]
        unsafe fn computer_will_wake(&self, notification: &NSNotification) {
            debug!("Received Wake Notification! {:?}", notification);
            let (tx, _rx) = oneshot::channel();
            let _ = self.ivars().global_tx.try_send(EventTriggers::Wake(tx));
        }

        #[unsafe(method(screenIsLocked:))]
        unsafe fn screen_is_locked(&self, notification: &NSNotification) {
            debug!("Received Lock Notification.. {:?}", notification);
            let _ = self.ivars().global_tx.try_send(EventTriggers::Lock);
        }

        #[unsafe(method(screenIsUnlocked:))]
        unsafe fn screen_is_unlocked(&self, notification: &NSNotification) {
            debug!("Received Unlock Notification.. {:?}", notification);
            let _ = self.ivars().global_tx.try_send(EventTriggers::Unlock);
        }
    }
}

impl UtilityDelegate {
    fn new(
        mtm: MainThreadMarker,
        sender: Sender<TrayOption>,
        global_tx: Sender<EventTriggers>,
        shutdown_signal: Arc<AtomicBool>,
    ) -> Retained<Self> {
        let delegate = mtm.alloc().set_ivars(State {
            sender,
            global_tx,
            shutdown_signal,
        });

        unsafe { msg_send![super(delegate), init] }
    }
}
