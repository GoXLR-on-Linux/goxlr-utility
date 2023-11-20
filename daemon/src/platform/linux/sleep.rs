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

use anyhow::Result;
use log::debug;
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

pub async fn run() -> Result<()> {
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
            debug!("Error: {:?}", error);
        }
    }

    debug!("Awaiting Result from 'PrepareForSleep'");
    let mut result = manager.receive_prepare_for_sleep().await?;

    debug!("Entering Signal Loop..");
    // We're gonna simply block here for a while until we get our signal..
    while let Some(signal) = result.next().await {
        let arg = signal.args()?;
        debug!("Received Arg: {}", arg.sleep);

        if arg.sleep {
            debug!("Dropping Handle.");
            drop(inhibitor.take());
        } else {
            debug!("Taking New Handle..");
            if let Ok(descriptor) = manager.inhibit(what, who, why, mode).await {
                debug!("Inhibitor Successfully Replaced.");
                inhibitor.replace(descriptor);
            }
        }
    }

    inhibitor.take();
    debug!("End of Run");
    Ok(())
}
