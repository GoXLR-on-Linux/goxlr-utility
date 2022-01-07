use goxlr_usb::goxlr::GoXLR;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut goxlr = GoXLR::open()?;
    let interrupt_duration = Duration::from_secs(60);
    let sleep_duration = Duration::from_millis(100);

    loop {
        if goxlr.await_interrupt(interrupt_duration) {
            goxlr.get_button_states();
        }
        sleep(sleep_duration);
    }
}
