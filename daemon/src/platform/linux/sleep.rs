/* So the goal here is to leverage zbus to pull out org.freedesktop.login1.Manager and listen for
   the 'PrepareForSleep' signal, that should give us a boolean telling us if we're about to go to
   sleep, of if we've just woken up. From there, we can throw the relevant Sleep Event across
   and handle it.

   We should also consider holding a delay inhibitor in order to keep the system awake form long
   enough to actually perform actions on the GoXLR. So when we see PrepareForSleep(true), perform
   sleep actions then drop the inhibitor, and when we see PrepareForSleep(false), perform wake
   actions and grab a new inhibitor. NOTE: The inhibitors used *SHOULD* be delay ones, the util
   should not completely block sleeps from happening.

   Refs:
   https://www.freedesktop.org/wiki/Software/systemd/logind/
   https://www.freedesktop.org/wiki/Software/systemd/inhibit/
*/

use crate::events::EventTriggers;
use anyhow::Result;
use log::debug;
use tokio::sync::{mpsc, oneshot};
use zbus::export::futures_util::StreamExt;
use zbus::zvariant::OwnedFd;
use zbus::{dbus_proxy, Connection};

#[dbus_proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    /// The method used to 'prevent' sleep until we're done..
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;

    /// The Sleep Signal Sent to us from DBus
    #[dbus_proxy(signal)]
    fn prepare_for_sleep(sleep: bool) -> Result<()>;
}

pub async fn run(tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    let mut inhibitor = None;

    debug!("Spawning Sleep Handler..");
    let conn = Connection::system().await?;
    let manager = ManagerProxy::new(&conn).await?;

    debug!("Attempting to Inhibit Sleep..");

    // We need to temporarily hold on these events, to allow us to load settings.
    let what = "sleep";
    let who = "GoXLR Utility";
    let why = "Applying Sleep Settings";
    let mode = "delay";

    match manager.inhibit(what, who, why, mode).await {
        Ok(descriptor) => {
            debug!("Inhibitor Successfully Established.");
            inhibitor.replace(descriptor);
        }
        Err(error) => {
            debug!("Unable to Create Inhibitor: {:?}", error);
        }
    }

    debug!("Awaiting Result from 'PrepareForSleep'");
    let mut result = manager.receive_prepare_for_sleep().await?;

    debug!("Entering Signal Loop..");
    // We're gonna simply block here for a while until we get our signal..
    while let Some(signal) = result.next().await {
        let arg = signal.args()?;
        if arg.sleep {
            debug!("Going to Sleep, Letting the Primary Worker know...");
            let (sleep_tx, sleep_rx) = oneshot::channel();

            if tx.send(EventTriggers::Sleep(sleep_tx)).await.is_ok() {
                // Wait for a Response back..
                debug!("Sleep Message Sent, awaiting completion..");
                let _ = sleep_rx.await;
            }

            debug!("Sleep Handling Complete, Attempting to Drop Inhibitor");
            if let Some(handle) = inhibitor.take() {
                debug!("Inhibitor Found, Dropping...");
                drop(handle);
            } else {
                debug!("No Inhibitor Present, hope for the best!");
            }
        } else {
            debug!("Waking Up, Letting Primary Worker Know...");

            let (wake_tx, wake_rx) = oneshot::channel();
            if tx.send(EventTriggers::Wake(wake_tx)).await.is_ok() {
                debug!("Wake Message Sent, awaiting completion..");
                let _ = wake_rx.await;
            }

            debug!("Wake Handling Complete, Attempting to replace Inhibitor");
            if let Ok(descriptor) = manager.inhibit(what, who, why, mode).await {
                debug!("Inhibitor Successfully Replaced");
                inhibitor.replace(descriptor);
            } else {
                debug!("Unable to create Inhibitor");
            }
        }
    }

    inhibitor.take();
    debug!("End of Run");
    Ok(())
}
