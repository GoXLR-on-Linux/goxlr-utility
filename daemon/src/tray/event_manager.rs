use crate::Shutdown;
use log::debug;
use tokio::select;
use tokio::sync::mpsc::Receiver;

pub enum Message {
    Open,
    Exit,
}

pub async fn start_event_handler(mut shutdown: Shutdown, mut rx: Receiver<Message>) {
    debug!("Starting Event Manager..");
    loop {
        select!(
            () = shutdown.recv() => {
                break;
            },
            Some(message) = rx.recv() => {
                match message {
                    Message::Open => {
                        debug!("Open Received..");

                        // TODO: Port / Bind Address
                        let _ = opener::open("http://localhost:14564/");
                    },
                    Message::Exit => {},
                }
            }
        )
    }
    debug!("Event Manager Ended..");
}
