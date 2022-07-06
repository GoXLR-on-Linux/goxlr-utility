use std::ops::DerefMut;
use actix_plus_static_files::{build_hashmap_from_included_dir, ResourceFiles, include_dir, Dir};
use actix_web::{App, get, post, HttpResponse, web, HttpServer};
use actix_web::dev::ServerHandle;
use actix_web::web::Data;

use anyhow::{anyhow, Result};
use futures::lock::Mutex;
use log::{debug, warn};
use strum::IntoEnumIterator;
use tokio::sync::oneshot::Sender;

use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand};
use goxlr_types::{ChannelName, CompressorAttackTime, CompressorRatio, CompressorReleaseTime, FaderName, GateTimes, InputDevice, MuteFunction, OutputDevice};

use crate::communication::handle_packet;
use crate::primary_worker::DeviceSender;

const WEB_CONTENT: Dir = include_dir!("./web-content/");

static version: f64 = 0.3;

pub async fn launch_httpd(usb_tx: DeviceSender, handle_tx: Sender<ServerHandle>) -> Result<()> {
    let server = HttpServer::new(move || {
        let static_files = build_hashmap_from_included_dir(&WEB_CONTENT);
        App::new()
            .app_data(Data::new(Mutex::new(usb_tx.clone())))
            .service(get_devices)
            .service(set_volume)
            .service(get_devices)
            .service(set_volume)
            .service(set_fader_channel)
            .service(set_fader_mute_function)
            .service(set_routing)
            .service(set_profile)
            .service(set_cough_behaviour)
            .service(set_compressor_threshold)
            .service(set_compressor_ratio)
            .service(set_compressor_attack)
            .service(set_compressor_release)
            .service(set_compressor_makeup)
            .service(set_noise_gate_threshold)
            .service(set_noise_gate_attenuation)
            .service(set_noise_gate_attack)
            .service(set_noise_gate_release)
            .service(ResourceFiles::new("/", static_files))
    })
        .bind(("127.0.0.1", 14564))?
        .run();
    let _ = handle_tx.send(server.handle());
    server.await?;
    Ok(())
}

#[get("/api/get-devices")]
async fn get_devices(usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    if let Ok(response) = get_status(usb_mutex).await {
        return HttpResponse::Ok().json(&response);
    }
    HttpResponse::InternalServerError().finish()

}


/**
 API / IPC related stuff, I know that you shouldn't really send parameters as URL segments,
 however, I'm using it to get some quick and easy type coercion, rather than having to create
 structs for processing form data / json on the incoming data.
*/

#[post("/api/set-volume/{serial}/{channel}/{volume}")]
async fn set_volume(path: web::Path<(String, u8, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, channel, volume) = path.into_inner();
    if let Some(channel_name) = ChannelName::iter().nth(channel.into()) {
        return send_cmd(usb_mutex, serial,
            GoXLRCommand::SetVolume(channel_name,volume)
        ).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-fader-channel/{serial}/{fader}/{channel}")]
async fn set_fader_channel(path: web::Path<(String, u8, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, fader, channel) = path.into_inner();
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(channel) = ChannelName::iter().nth(channel.into()) {
            return send_cmd(usb_mutex, serial,
                GoXLRCommand::SetFader(fader,channel)
            ).await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-fader-mute/{serial}/{fader}/{function}")]
async fn set_fader_mute_function(path: web::Path<(String, u8, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, fader, function) = path.into_inner();
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(function) = MuteFunction::iter().nth(function.into()) {
            return send_cmd(usb_mutex, serial,
                GoXLRCommand::SetFaderMuteFunction(fader,function)
            ).await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-routing/{serial}/{input}/{output}/{value}")]
async fn set_routing(path: web::Path<(String, u8, u8, bool)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, input, output, value) = path.into_inner();
    if let Some(input) = InputDevice::iter().nth(input.into()) {
        if let Some(output) = OutputDevice::iter().nth(output.into()) {
            return send_cmd(usb_mutex, serial,
                GoXLRCommand::SetRouter(input,output,value)
            ).await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-profile/{serial}/{profile_name}")]
async fn set_profile(path: web::Path<(String, String)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, profile_name) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::LoadProfile(profile_name)).await;
}

#[post("/api/set-cough-behaviour/{serial}/{function}")]
async fn set_cough_behaviour(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, function) = path.into_inner();
    if let Some(function) = MuteFunction::iter().nth(function.into()) {
        return send_cmd(usb_mutex, serial,
            GoXLRCommand::SetCoughMuteFunction(function)
        ).await;
    }
    HttpResponse::InternalServerError().finish()
}

/** Compressor **/
#[post("/api/set-compressor-threshold/{serial}/{value}")]
async fn set_compressor_threshold(path: web::Path<(String, i8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetCompressorThreshold(value)).await;
}

#[post("/api/set-compressor-ratio/{serial}/{value}")]
async fn set_compressor_ratio(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(ratio) = CompressorRatio::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial,
            GoXLRCommand::SetCompressorRatio(ratio)
        ).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-attack/{serial}/{value}")]
async fn set_compressor_attack(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(attack) = CompressorAttackTime::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial,
            GoXLRCommand::SetCompressorAttack(attack)
        ).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-release/{serial}/{value}")]
async fn set_compressor_release(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(release) = CompressorReleaseTime::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial,
            GoXLRCommand::SetCompressorReleaseTime(release)
        ).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-makeup/{serial}/{value}")]
async fn set_compressor_makeup(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetCompressorMakeupGain(value)).await;
}

/** Gate **/
#[post("/api/set-noise-gate-threshold/{serial}/{value}")]
async fn set_noise_gate_threshold(path: web::Path<(String, i8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateThreshold(value)).await;
}

#[post("/api/set-noise-gate-attenuation/{serial}/{value}")]
async fn set_noise_gate_attenuation(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateAttenuation(value)).await;
}

#[post("/api/set-noise-gate-attack/{serial}/{value}")]
async fn set_noise_gate_attack(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(attack) = GateTimes::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateAttack(attack)).await;
    }

    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-noise-gate-release/{serial}/{value}")]
async fn set_noise_gate_release(path: web::Path<(String, u8)>, usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(release) = GateTimes::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateRelease(release)).await;
    }

    HttpResponse::InternalServerError().finish()
}


async fn send_cmd(usb_tx: Data<Mutex<DeviceSender>>, serial: String, command: GoXLRCommand) -> HttpResponse {
    debug!("Request: {:?}", command.clone());

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
        return HttpResponse::InternalServerError().finish()
    }

    return HttpResponse::Ok().finish();
}

async fn get_status(usb_tx: Data<Mutex<DeviceSender>>) -> Result<DaemonStatus> {
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