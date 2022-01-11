use tokio::sync::broadcast;

pub struct Shutdown {
    shutdown: bool,
    sender: broadcast::Sender<()>,
    receiver: broadcast::Receiver<()>,
}

impl Shutdown {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel(1);
        Self {
            shutdown: false,
            sender,
            receiver,
        }
    }

    pub fn trigger(&self) {
        let _ = self.sender.send(());
    }

    pub async fn recv(&mut self) {
        if self.shutdown {
            return;
        }

        let _ = self.receiver.recv().await;
        self.shutdown = true;
    }
}

impl Clone for Shutdown {
    fn clone(&self) -> Self {
        let sender = self.sender.clone();
        let receiver = self.sender.subscribe();
        Self {
            shutdown: self.shutdown,
            sender,
            receiver,
        }
    }
}
