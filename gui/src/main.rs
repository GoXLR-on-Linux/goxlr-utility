mod client;

use crate::client::Client;
use anyhow::{Context, Result};
use cpp_core::{Ptr, StaticUpcast};
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DeviceType, GoXLRCommand, MixerStatus, UsbProductInformation,
};
use goxlr_ipc::{DeviceStatus, Socket};
use goxlr_types::{ChannelName, FaderName};
use qt_core::{q_init_resource, qs, slot, QBox, QObject, QPtr, SlotNoArgs, SlotOfInt};
use qt_ui_tools::ui_form;
use qt_widgets::{QApplication, QComboBox, QSlider, QSpinBox, QWidget};
use std::borrow::{Borrow, BorrowMut};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use strum::IntoEnumIterator;
use tokio::net::UnixStream;
use tokio::runtime::Runtime;
use tokio::task::block_in_place;

#[ui_form("../assets/goxlr.ui")]
#[derive(Debug)]
struct Form {
    widget: QBox<QWidget>,

    // Ignore any warnings about snake case here, these map by name to their
    // relevant form control..
    fader_a: QPtr<QComboBox>,
    fader_b: QPtr<QComboBox>,
    fader_c: QPtr<QComboBox>,
    fader_d: QPtr<QComboBox>,

    chat_slider: QPtr<QSlider>,
    console_slider: QPtr<QSlider>,
    game_slider: QPtr<QSlider>,
    linein_slider: QPtr<QSlider>,
    mic_slider: QPtr<QSlider>,
    music_slider: QPtr<QSlider>,
    sample_slider: QPtr<QSlider>,
    system_slider: QPtr<QSlider>,

    chat_spin: QPtr<QSpinBox>,
    console_spin: QPtr<QSpinBox>,
    game_spin: QPtr<QSpinBox>,
    linein_spin: QPtr<QSpinBox>,
    mic_spin: QPtr<QSpinBox>,
    music_spin: QPtr<QSpinBox>,
    sample_spin: QPtr<QSpinBox>,
    system_spin: QPtr<QSpinBox>,
}

struct GoXLR {
    // We may need other models later, so keeping this open..
    form: Form,
}

impl StaticUpcast<QObject> for GoXLR {
    unsafe fn static_upcast(ptr: Ptr<Self>) -> Ptr<QObject> {
        ptr.form.widget.as_ptr().static_upcast()
    }
}

impl GoXLR {
    fn new() -> Rc<Self> {
        unsafe {
            let this = Rc::new(GoXLR { form: Form::load() });
            this.init();
            this
        }
    }

    unsafe fn init(self: &Rc<Self>) {
        // These are a little annoying, I'm not sure if it's possible to extract the combobox which
        // was changed from the method call, so for now we'll just smash them into different methods.
        self.form
            .fader_a
            .current_index_changed()
            .connect(&self.slot_on_fader_a_changed());
        self.form
            .fader_b
            .current_index_changed()
            .connect(&self.slot_on_fader_b_changed());
        self.form
            .fader_c
            .current_index_changed()
            .connect(&self.slot_on_fader_c_changed());
        self.form
            .fader_d
            .current_index_changed()
            .connect(&self.slot_on_fader_d_changed());

        // Same Here :p
        self.form
            .chat_slider
            .value_changed()
            .connect(&self.slot_on_chat_slider_moved());
        self.form
            .console_slider
            .value_changed()
            .connect(&self.slot_on_console_slider_moved());
        self.form
            .game_slider
            .value_changed()
            .connect(&self.slot_on_game_slider_moved());
        self.form
            .linein_slider
            .value_changed()
            .connect(&self.slot_on_line_in_slider_moved());
        self.form
            .mic_slider
            .value_changed()
            .connect(&self.slot_on_mic_slider_moved());
        self.form
            .music_slider
            .value_changed()
            .connect(&self.slot_on_music_slider_moved());
        self.form
            .sample_slider
            .value_changed()
            .connect(&self.slot_on_sample_slider_moved());
        self.form
            .system_slider
            .value_changed()
            .connect(&self.slot_on_system_slider_moved());

        self.form
            .chat_spin
            .value_changed()
            .connect(&self.slot_on_chat_spin_change());
        self.form
            .console_spin
            .value_changed()
            .connect(&self.slot_on_console_spin_change());
        self.form
            .game_spin
            .value_changed()
            .connect(&self.slot_on_game_spin_change());
        self.form
            .linein_spin
            .value_changed()
            .connect(&self.slot_on_line_in_spin_change());
        self.form
            .mic_spin
            .value_changed()
            .connect(&self.slot_on_mic_spin_change());
        self.form
            .music_spin
            .value_changed()
            .connect(&self.slot_on_music_spin_change());
        self.form
            .sample_spin
            .value_changed()
            .connect(&self.slot_on_sample_spin_change());
        self.form
            .system_spin
            .value_changed()
            .connect(&self.slot_on_system_spin_change());
    }

    #[slot(SlotNoArgs)]
    unsafe fn on_fader_a_changed(self: &Rc<Self>) {
        dbg!("Fader A Changed..");
    }

    #[slot(SlotNoArgs)]
    unsafe fn on_fader_b_changed(self: &Rc<Self>) {
        dbg!("Fader B Changed..");
    }

    #[slot(SlotNoArgs)]
    unsafe fn on_fader_c_changed(self: &Rc<Self>) {
        dbg!("Fader C Changed..");
    }

    #[slot(SlotNoArgs)]
    unsafe fn on_fader_d_changed(self: &Rc<Self>) {
        dbg!("Fader D Changed..");
    }

    #[slot(SlotOfInt)]
    unsafe fn on_chat_slider_moved(self: &Rc<Self>, value: i32) {
        set_slider_sync(ChannelName::Chat, value as u8);

        // Calculate the percentage (this needs fixing, it rounds down..)
        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }

        // Prevent QT from sending a 'Changed' value on the spinner while we update..
        self.form.chat_spin.block_signals(true);
        self.form.chat_spin.set_value(percent);
        self.form.chat_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_console_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("Console Slider Moved {}:", value);
        set_slider_sync(ChannelName::Console, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.console_spin.block_signals(true);
        self.form.console_spin.set_value(percent);
        self.form.console_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_game_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("Game Slider Moved {}:", value);
        set_slider_sync(ChannelName::Game, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.game_spin.block_signals(true);
        self.form.game_spin.set_value(percent);
        self.form.game_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_line_in_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("LineIn Slider Moved {}:", value);
        set_slider_sync(ChannelName::LineIn, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.linein_spin.block_signals(true);
        self.form.linein_spin.set_value(percent);
        self.form.linein_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_mic_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("Mic Slider Moved {}:", value);
        set_slider_sync(ChannelName::Mic, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.mic_spin.block_signals(true);
        self.form.mic_spin.set_value(percent);
        self.form.mic_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_music_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("Music Slider Moved {}:", value);
        set_slider_sync(ChannelName::Music, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.music_spin.block_signals(true);
        self.form.music_spin.set_value(percent);
        self.form.music_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_sample_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("Sample Slider Moved {}:", value);
        set_slider_sync(ChannelName::Sample, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.sample_spin.block_signals(true);
        self.form.sample_spin.set_value(percent);
        self.form.sample_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_system_slider_moved(self: &Rc<Self>, value: i32) {
        dbg!("System Slider Moved {}:", value);
        set_slider_sync(ChannelName::System, value as u8);

        let mut percent = (((value * 1000) / 255) * 100) / 1000;
        if percent != 0 {
            percent += 1
        }
        self.form.system_spin.block_signals(true);
        self.form.system_spin.set_value(percent);
        self.form.system_spin.block_signals(false);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_chat_spin_change(self: &Rc<Self>, value: i32) {
        // Rather than dealing with floats, multiply stuff..
        let out = (((value * 1000) / 100) * 255) / 1000;

        // Calling this will also call on_chat_slider_moved, which will update the GoXLR
        self.form.chat_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_console_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.console_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_game_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.game_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_line_in_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.linein_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_mic_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.mic_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_music_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.music_slider.set_slider_position(out);
    }
    #[slot(SlotOfInt)]
    unsafe fn on_sample_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.sample_slider.set_slider_position(out);
    }

    #[slot(SlotOfInt)]
    unsafe fn on_system_spin_change(self: &Rc<Self>, value: i32) {
        let out = (((value * 1000) / 100) * 255) / 1000;
        self.form.system_slider.set_slider_position(out);
    }

    fn show(self: &Rc<Self>) {
        unsafe {
            self.form.widget.show();
        }
    }
}

fn set_slider_sync(channel: ChannelName, value: u8) {
    thread::spawn(move || unsafe {
        set_slider(channel, value);
    })
    .join()
    .expect("Thread Panicked");
}

#[tokio::main]
async unsafe fn set_slider(channel: ChannelName, value: u8) {
    CLIENT
        .as_mut()
        .unwrap()
        .send(GoXLRCommand::SetVolume(channel, value))
        .await
        .context("Couldn't set Slider");
}

// F*** it, we're gonna do some unsafe globalisation here, until I can work out a less
// painful way to handle this..
static mut STREAM: Option<UnixStream> = None;
static mut CLIENT: Option<Client> = None;
static mut SOCKET: Option<Socket<DaemonResponse, DaemonRequest>> = None;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to the GoXLR..

    let mut stream = UnixStream::connect("/tmp/goxlr.socket")
        .await
        .context("Could not connect to the GoXLR Daemon Socket")?;

    let address = stream
        .peer_addr()
        .context("Could not get the address of the GoXLR daemon process")?;

    unsafe {
        STREAM = Some(stream);
        //SOCKET = Some(Socket::new(address, STREAM.borrow_mut().as_mut().unwrap()));
        let socket = Socket::new(address, STREAM.borrow_mut().as_mut().unwrap());
        CLIENT = Some(Client::new(socket));

        CLIENT
            .as_mut()
            .unwrap()
            .send(GoXLRCommand::GetStatus)
            .await
            .context("Couldn't retrieve device status..");

        print_device(CLIENT.as_ref().unwrap().device());
    }

    //let mut client = Client::new(socket);

    QApplication::init(|_| {
        q_init_resource!("resources");
        let goxlr_gui = GoXLR::new();
        goxlr_gui.show();
        unsafe { QApplication::exec() }
    });

    dbg!("Hi?");
    // Technically, this line is unreachable due to initing the QApplication..
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

    if let Some(mixer) = &device.mixer {
        print_mixer_info(mixer);
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

fn print_mixer_info(mixer: &MixerStatus) {
    for fader in FaderName::iter() {
        println!(
            "Fader {:?} assignment: {:?}",
            fader,
            mixer.get_fader_assignment(fader)
        )
    }

    for channel in ChannelName::iter() {
        let pct = (mixer.get_channel_volume(channel) as f32 / 255.0) * 100.0;
        if mixer.get_channel_muted(channel) {
            println!("{} volume: {:.0}% (Muted)", channel, pct);
        } else {
            println!("{} volume: {:.0}%", channel, pct);
        }
    }
}
