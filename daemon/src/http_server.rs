use std::ops::DerefMut;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use futures::lock::Mutex;
use include_dir::{Dir, include_dir};
use log::{debug, warn};
use rocket::{Ignite, Rocket, routes, State};
use rocket::{get, post};
use rocket::fs::{FileServer, NamedFile};
use rocket::http::{ContentType, Status};
use strum::IntoEnumIterator;
use tokio::net::UnixStream;

use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, Socket};
use goxlr_ipc::client::Client;
use goxlr_types::{ChannelName, CompressorAttackTime, CompressorRatio, CompressorReleaseTime, FaderName, GateTimes, InputDevice, MuteFunction, OutputDevice};

use crate::communication::handle_packet;
use crate::primary_worker::DeviceSender;

static WEB_CONTENT: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web-content/");

pub struct HttpServer {
}

impl HttpServer {
    pub async fn launch(usb_tx: DeviceSender) -> Result<()> {

        let client = HttpServer::connect_to_daemon().await?;

        let rocket = rocket::build()
            .manage(Mutex::new(usb_tx))
            .mount("/", routes![files])
            .mount("/api/", routes![
                get_devices,
                set_volume,
                set_fader_channel,
                set_fader_mute_function,
                set_routing,
                set_profile,
                set_cough_behaviour,
                set_compressor_threshold,
                set_compressor_ratio,
                set_compressor_attack,
                set_compressor_release,
                set_compressor_makeup,
                set_noise_gate_threshold,
                set_noise_gate_attenuation,
                set_noise_gate_attack,
                set_noise_gate_release,
            ])
            .ignite().await?
            .launch().await?;
        Ok(())
    }

    async fn connect_to_daemon() -> Result<Client> {
        let stream = UnixStream::connect("/tmp/goxlr.socket")
            .await
            .context("Could not connect to the GoXLR Daemon Socket")?;

        let address = stream
            .peer_addr()
            .context("Could not get the address of the GoXLR daemon process")?;

        let socket = Socket::new(address, stream);
        let mut client = Client::new(socket);
        client.poll_status().await?;

        Ok(client)
    }

    // pub async fn shutdown(&self) {
    //     debug!("Attempting HTTP Shutdown..");
    //     self.rocket.shutdown().notify();
    // }
}

#[get("/get-devices")]
async fn get_devices(state: &State<Mutex<DeviceSender>>) -> Result<String, Status> {
    if let Ok(response) = get_status(state).await {
        return Ok(serde_json::to_string(&response).unwrap());
    }
    Err(Status::InternalServerError)
}

#[get("/<file..>")]
async fn files(mut file: PathBuf) -> (Status, (ContentType, &'static str)) {
    if file.as_os_str().is_empty() {
        file = PathBuf::from("index.html");
    }

    // Attempt to determine the content type..
    let mut content_type: ContentType = ContentType::Plain;
    if let Some(ext) = file.extension() {
        if let Some(ct) = ContentType::from_extension(&ext.to_string_lossy()) {
            content_type = ct;
        }
    }

    // Ok, try and find this file in our embedded data..
    if let Some(file) = WEB_CONTENT.get_file(file) {
        if let Some(content) = file.contents_utf8() {
            return (Status::Ok, (content_type, content));
        }
    }
    return (Status::NotFound, (ContentType::Plain, "File not Found"));
}

/**
 API / IPC related stuff, I know that you shouldn't really send parameters as URL segments,
 however, I'm using it to get some quick and easy type coercion, rather than having to create
 structs for processing form data / json on the incoming data.
*/

#[post("/set-volumes/<serial>/<channel>/<volume>")]
async fn set_volume(serial: String, channel: u8, volume: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(channel_name) = ChannelName::iter().nth(channel.into()) {
        return send_cmd(state, serial,
            GoXLRCommand::SetVolume(channel_name,volume)
        ).await;
    }
    Status::InternalServerError
}

#[post("/set-fader-channel/<serial>/<fader>/<channel>")]
async fn set_fader_channel(serial: String, fader: u8, channel: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(channel) = ChannelName::iter().nth(channel.into()) {
            return send_cmd(state, serial,
                GoXLRCommand::SetFader(fader,channel)
            ).await;
        }
    }
    Status::InternalServerError
}

#[post("/set-fader-mute/<serial>/<fader>/<function>")]
async fn set_fader_mute_function(serial: String, fader: u8, function: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(function) = MuteFunction::iter().nth(function.into()) {
            return send_cmd(state, serial,
                GoXLRCommand::SetFaderMuteFunction(fader,function)
            ).await;
        }
    }
    Status::InternalServerError
}

#[post("/set-routing/<serial>/<input>/<output>/<value>")]
async fn set_routing(serial: String, input: u8, output: u8, value: bool, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(input) = InputDevice::iter().nth(input.into()) {
        if let Some(output) = OutputDevice::iter().nth(output.into()) {
            return send_cmd(state, serial,
                GoXLRCommand::SetRouter(input,output,value)
            ).await;
        }
    }
    Status::InternalServerError
}

#[post("/set-profile/<serial>/<profile_name>")]
async fn set_profile(serial: String, profile_name: String, state: &State<Mutex<DeviceSender>>) -> Status {
    return send_cmd(state, serial, GoXLRCommand::LoadProfile(profile_name)).await;
}

#[post("/set-cough-behaviour/<serial>/<function>")]
async fn set_cough_behaviour(serial: String, function: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(function) = MuteFunction::iter().nth(function.into()) {
        return send_cmd(state, serial,
            GoXLRCommand::SetCoughMuteFunction(function)
        ).await;
    }
    Status::InternalServerError
}

/** Compressor **/
#[post("/set-compressor-threshold/<serial>/<value>")]
async fn set_compressor_threshold(serial: String, value: i8, state: &State<Mutex<DeviceSender>>) -> Status {
    return send_cmd(state, serial, GoXLRCommand::SetCompressorThreshold(value)).await;
}

#[post("/set-compressor-ratio/<serial>/<value>")]
async fn set_compressor_ratio(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(ratio) = CompressorRatio::iter().nth(value.into()) {
        return send_cmd(state, serial,
            GoXLRCommand::SetCompressorRatio(ratio)
        ).await;
    }
    Status::InternalServerError
}

#[post("/set-compressor-attack/<serial>/<value>")]
async fn set_compressor_attack(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(attack) = CompressorAttackTime::iter().nth(value.into()) {
        return send_cmd(state, serial,
            GoXLRCommand::SetCompressorAttack(attack)
        ).await;
    }
    Status::InternalServerError
}

#[post("/set-compressor-release/<serial>/<value>")]
async fn set_compressor_release(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(release) = CompressorReleaseTime::iter().nth(value.into()) {
        return send_cmd(state, serial,
            GoXLRCommand::SetCompressorReleaseTime(release)
        ).await;
    }
    Status::InternalServerError
}

#[post("/set-compressor-makeup/<serial>/<value>")]
async fn set_compressor_makeup(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    return send_cmd(state, serial, GoXLRCommand::SetCompressorMakeupGain(value)).await;
}

/** Gate **/
#[post("/set-noise-gate-threshold/<serial>/<value>")]
async fn set_noise_gate_threshold(serial: String, value: i8, state: &State<Mutex<DeviceSender>>) -> Status {
    return send_cmd(state, serial, GoXLRCommand::SetGateThreshold(value)).await;
}

#[post("/set-noise-gate-attenuation/<serial>/<value>")]
async fn set_noise_gate_attenuation(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    return send_cmd(state, serial, GoXLRCommand::SetGateAttenuation(value)).await;
}

#[post("/set-noise-gate-attack/<serial>/<value>")]
async fn set_noise_gate_attack(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(attack) = GateTimes::iter().nth(value.into()) {
        return send_cmd(state, serial, GoXLRCommand::SetGateAttack(attack)).await;
    }

    Status::InternalServerError
}

#[post("/set-noise-gate-release/<serial>/<value>")]
async fn set_noise_gate_release(serial: String, value: u8, state: &State<Mutex<DeviceSender>>) -> Status {
    if let Some(release) = GateTimes::iter().nth(value.into()) {
        return send_cmd(state, serial, GoXLRCommand::SetGateRelease(release)).await;
    }

    Status::InternalServerError
}

async fn send_cmd(usb_tx: &State<Mutex<DeviceSender>>, serial: String, command: GoXLRCommand) -> Status {
    // Unwrap the Mutex Guard..
    let mut guard = usb_tx.lock().await;
    let sender = guard.deref_mut();

    // Prepare the Command..
    let request = DaemonRequest::Command {
        0: serial,
        1: command
    };

    // Because most request are going to either send a 200 Ok, or 500 Internal Server error,
    // we might as well intercept any errors here, and straight up return the status.
    let result = handle_packet(request, sender).await;
    if result.is_err() {
        warn!("Error Handling Request, {:?}", result.as_ref().err());
        return Status::InternalServerError;
    }

    return Status::Ok;
}

async fn get_status(usb_tx: &State<Mutex<DeviceSender>>) -> Result<DaemonStatus> {
    // Unwrap the Mutex Guard..
    let mut guard = usb_tx.lock().await;
    let sender = guard.deref_mut();

    let request = DaemonRequest::GetStatus;

    let result = handle_packet(request, sender).await?;
    return match result {
        DaemonResponse::Status(status) => {
            Ok(status)
        }
        _ => {
            Err(anyhow!("Unexpected Daemon Status Result: {:?}", result))
        }
    }
}