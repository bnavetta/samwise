use tokio::sync::watch;

/// Helper for graceful shutdown
#[derive(Clone)]
pub struct Shutdown {
    receiver: watch::Receiver<bool>,
    shutdown: bool,
}

impl Shutdown {
    pub fn new(receiver: watch::Receiver<bool>) -> Shutdown {
        Shutdown {
            receiver,
            shutdown: false,
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    pub async fn recv(&mut self) -> bool {
        if self.shutdown {
            return true;
        }

        match self.receiver.recv().await {
            Some(v) => {
                self.shutdown = v;
                v
            }
            None => {
                // If the sender is shut down, then assume we're shutting down as we'll never
                // receive another signal
                self.shutdown = true;
                true
            }
        }
    }
}
