/* So the goal here is to leverage zbus to pull out org.freedesktop.login1.Manager and listen for
   the 'PrepareForSleep' signal, that should give us a boolean telling us if we're about to go to
   sleep, of if we've just woken up. From there, we can throw the relevant Sleep Event across
   and handle it.

   We also implement Sleep inhibitors so we can prevent the sleep from occurring until we've
   confirmed that our sleep actions have been performed.

   Refs:
   https://www.freedesktop.org/wiki/Software/systemd/logind/
   https://www.freedesktop.org/wiki/Software/systemd/inhibit/
*/

use std::collections::HashMap;
use std::env;

use anyhow::Result;
use log::{debug, warn};
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use zbus::export::futures_util::StreamExt;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};
use zbus::{proxy, Connection};

use crate::events::EventTriggers;
use crate::shutdown::Shutdown;

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    /// The method used to 'prevent' sleep until we're done..
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;

    /// Get a session..
    fn get_session(&self, session_id: &str) -> zbus::Result<OwnedObjectPath>;

    /// The Sleep Signal Sent to us from DBus
    #[zbus(signal)]
    fn prepare_for_sleep(sleep: bool) -> Result<()>;
}

#[proxy(
    interface = "org.freedesktop.DBus.Properties",
    default_service = "org.freedesktop.login1"
)]
trait Session {
    #[zbus(signal)]
    fn properties_changed(
        interface: String,
        changed: HashMap<String, OwnedValue>,
        invalidated: Vec<String>,
    ) -> Result<()>;
}

pub async fn run(tx: mpsc::Sender<EventTriggers>, mut stop: Shutdown) -> Result<()> {
    let mut inhibitor = None;

    debug!("Spawning Sleep Handler..");
    let conn = Connection::system().await?;
    let manager = ManagerProxy::new(&conn).await?;

    let session_proxy = if let Ok(value) = env::var("XDG_SESSION_ID") {
        if let Ok(session) = manager.get_session(&value).await {
            if let Ok(builder) = SessionProxy::builder(&conn).path(session) {
                builder.build().await.ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if session_proxy.is_none() {
        warn!("Unable to setup Lock / Unlock dbus register");
    }

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

    let mut sleep_result = manager.receive_prepare_for_sleep().await?;

    debug!("Preparing Lock Proxy..");
    let mut lock_result = if let Some(proxy) = session_proxy {
        Some(proxy.receive_properties_changed().await?)
    } else {
        None
    };

    debug!("Entering Signal Loop..");
    // We'll use a tokio select() to work out the signals..
    loop {
        select! {
            Some(signal) = sleep_result.next() => {
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
            Some(signal) = conditional(&mut lock_result) => {
                let args = signal.args().unwrap();

                if args.changed.contains_key("LockedHint") {
                    let value = bool::try_from(args.changed.get("LockedHint").unwrap())?;
                    if value {
                        let _ = tx.send(EventTriggers::Lock).await;
                    } else {
                        let _ = tx.send(EventTriggers::Unlock).await;
                    }
                }
            },
            () = stop.recv() => {
                break;
            }
        }
    }

    inhibitor.take();
    debug!("End of Run");
    Ok(())
}

// This is a simple method to handle whether the conditional properties haven't been set..
async fn conditional(t: &mut Option<PropertiesChangedStream<'_>>) -> Option<PropertiesChanged> {
    match t {
        Some(ref mut change) => change.next().await,
        None => None,
    }
}
