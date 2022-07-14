use actix::{
    Actor, ActorContext, AsyncContext, ContextFutureSpawner, Handler, Message, StreamHandler,
    WrapFuture,
};
use actix_cors::Cors;
use actix_plus_static_files::{build_hashmap_from_included_dir, include_dir, Dir, ResourceFiles};
use actix_web::dev::ServerHandle;
use actix_web::web::Data;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use actix_web_actors::ws::CloseCode;
use std::ops::DerefMut;

use anyhow::{anyhow, Result};
use futures::lock::Mutex;
use log::{debug, warn};
use strum::IntoEnumIterator;
use tokio::sync::oneshot::Sender;

use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand};
use goxlr_types::{
    ChannelName, CompressorAttackTime, CompressorRatio, CompressorReleaseTime, FaderName,
    GateTimes, InputDevice, MuteFunction, OutputDevice,
};

use crate::communication::handle_packet;
use crate::primary_worker::DeviceSender;

const WEB_CONTENT: Dir = include_dir!("./web-content/");

struct Websocket {
    sender: DeviceSender,
}

impl Actor for Websocket {
    type Context = ws::WebsocketContext<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
struct WsResponse(DaemonResponse);

impl Handler<WsResponse> for Websocket {
    type Result = ();

    fn handle(&mut self, msg: WsResponse, ctx: &mut Self::Context) -> Self::Result {
        if let Ok(result) = serde_json::to_string(&msg.0) {
            ctx.text(result);
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Websocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_slice::<DaemonRequest>(text.as_ref()) {
                    Ok(request) => {
                        let recipient = ctx.address().recipient();
                        let mut usb_tx = self.sender.clone();
                        let future = async move {
                            let result = handle_packet(request, &mut usb_tx).await;
                            match result {
                                Ok(resp) => match resp {
                                    DaemonResponse::Ok => {}
                                    DaemonResponse::Error(error) => {
                                        recipient.do_send(WsResponse(DaemonResponse::Error(error)));
                                    }
                                    DaemonResponse::Status(status) => {
                                        recipient
                                            .do_send(WsResponse(DaemonResponse::Status(status)));
                                    }
                                },
                                Err(error) => {
                                    recipient.do_send(WsResponse(DaemonResponse::Error(
                                        error.to_string(),
                                    )));
                                }
                            }
                        };
                        future.into_actor(self).spawn(ctx);
                    }
                    Err(error) => {
                        warn!("HTTP Error: {}", error);
                        warn!("Request: {}", text);
                        ctx.close(Some(CloseCode::Invalid.into()));
                        ctx.stop();
                    }
                }
            }
            Ok(ws::Message::Binary(_bin)) => {
                ctx.close(Some(CloseCode::Unsupported.into()));
                ctx.stop();
            }
            _ => (),
        }
    }
}

pub async fn launch_httpd(
    usb_tx: DeviceSender,
    handle_tx: Sender<ServerHandle>,
    port: u16,
) -> Result<()> {
    let server = HttpServer::new(move || {
        let static_files = build_hashmap_from_included_dir(&WEB_CONTENT);
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1")
            .allowed_origin("http://localhost")
            .allowed_origin_fn(|origin, _req_head| {
                origin.as_bytes().starts_with(b"http://127.0.0.1")
                    || origin.as_bytes().starts_with(b"http://localhost")
            })
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        App::new()
            .wrap(cors)
            .app_data(Data::new(Mutex::new(usb_tx.clone())))
            .service(get_devices)
            .service(set_volume)
            .service(get_devices)
            .service(set_volume)
            .service(set_bleep_volume)
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
            .service(websocket)
            .service(ResourceFiles::new("/", static_files))
    })
    .bind(("127.0.0.1", port))?
    .run();
    let _ = handle_tx.send(server.handle());
    server.await?;
    Ok(())
}

#[get("/api/websocket")]
async fn websocket(
    usb_mutex: Data<Mutex<DeviceSender>>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(
        Websocket {
            sender: usb_mutex.lock().await.clone(),
        },
        &req,
        stream,
    )
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
async fn set_volume(
    path: web::Path<(String, u8, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, channel, volume) = path.into_inner();
    if let Some(channel_name) = ChannelName::iter().nth(channel.into()) {
        return send_cmd(
            usb_mutex,
            serial,
            GoXLRCommand::SetVolume(channel_name, volume),
        )
        .await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-fader-channel/{serial}/{fader}/{channel}")]
async fn set_fader_channel(
    path: web::Path<(String, u8, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, fader, channel) = path.into_inner();
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(channel) = ChannelName::iter().nth(channel.into()) {
            return send_cmd(usb_mutex, serial, GoXLRCommand::SetFader(fader, channel)).await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-fader-mute/{serial}/{fader}/{function}")]
async fn set_fader_mute_function(
    path: web::Path<(String, u8, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, fader, function) = path.into_inner();
    if let Some(fader) = FaderName::iter().nth(fader.into()) {
        if let Some(function) = MuteFunction::iter().nth(function.into()) {
            return send_cmd(
                usb_mutex,
                serial,
                GoXLRCommand::SetFaderMuteFunction(fader, function),
            )
            .await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-routing/{serial}/{input}/{output}/{value}")]
async fn set_routing(
    path: web::Path<(String, u8, u8, bool)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, input, output, value) = path.into_inner();
    if let Some(input) = InputDevice::iter().nth(input.into()) {
        if let Some(output) = OutputDevice::iter().nth(output.into()) {
            return send_cmd(
                usb_mutex,
                serial,
                GoXLRCommand::SetRouter(input, output, value),
            )
            .await;
        }
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-profile/{serial}/{profile_name}")]
async fn set_profile(
    path: web::Path<(String, String)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, profile_name) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::LoadProfile(profile_name)).await;
}

#[post("/api/set-cough-behaviour/{serial}/{function}")]
async fn set_cough_behaviour(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, function) = path.into_inner();
    if let Some(function) = MuteFunction::iter().nth(function.into()) {
        return send_cmd(
            usb_mutex,
            serial,
            GoXLRCommand::SetCoughMuteFunction(function),
        )
        .await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-bleep-volume/{serial}/{value}")]
async fn set_bleep_volume(
    path: web::Path<(String, i8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, function) = path.into_inner();
    return send_cmd(
        usb_mutex,
        serial,
        GoXLRCommand::SetSwearButtonVolume(function),
    )
    .await;
}

/** Compressor **/
#[post("/api/set-compressor-threshold/{serial}/{value}")]
async fn set_compressor_threshold(
    path: web::Path<(String, i8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(
        usb_mutex,
        serial,
        GoXLRCommand::SetCompressorThreshold(value),
    )
    .await;
}

#[post("/api/set-compressor-ratio/{serial}/{value}")]
async fn set_compressor_ratio(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(ratio) = CompressorRatio::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetCompressorRatio(ratio)).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-attack/{serial}/{value}")]
async fn set_compressor_attack(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(attack) = CompressorAttackTime::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetCompressorAttack(attack)).await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-release/{serial}/{value}")]
async fn set_compressor_release(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(release) = CompressorReleaseTime::iter().nth(value.into()) {
        return send_cmd(
            usb_mutex,
            serial,
            GoXLRCommand::SetCompressorReleaseTime(release),
        )
        .await;
    }
    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-compressor-makeup/{serial}/{value}")]
async fn set_compressor_makeup(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(
        usb_mutex,
        serial,
        GoXLRCommand::SetCompressorMakeupGain(value),
    )
    .await;
}

/** Gate **/
#[post("/api/set-noise-gate-threshold/{serial}/{value}")]
async fn set_noise_gate_threshold(
    path: web::Path<(String, i8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateThreshold(value)).await;
}

#[post("/api/set-noise-gate-attenuation/{serial}/{value}")]
async fn set_noise_gate_attenuation(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateAttenuation(value)).await;
}

#[post("/api/set-noise-gate-attack/{serial}/{value}")]
async fn set_noise_gate_attack(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(attack) = GateTimes::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateAttack(attack)).await;
    }

    HttpResponse::InternalServerError().finish()
}

#[post("/api/set-noise-gate-release/{serial}/{value}")]
async fn set_noise_gate_release(
    path: web::Path<(String, u8)>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let (serial, value) = path.into_inner();
    if let Some(release) = GateTimes::iter().nth(value.into()) {
        return send_cmd(usb_mutex, serial, GoXLRCommand::SetGateRelease(release)).await;
    }

    HttpResponse::InternalServerError().finish()
}

async fn send_cmd(
    usb_tx: Data<Mutex<DeviceSender>>,
    serial: String,
    command: GoXLRCommand,
) -> HttpResponse {
    debug!("Request: {:?}", command.clone());

    // Unwrap the Mutex Guard..
    let mut guard = usb_tx.lock().await;
    let sender = guard.deref_mut();

    // Prepare the Command..
    let request = DaemonRequest::Command(serial, command);

    debug!("Command: {}", serde_json::to_string(&request).unwrap());

    // Because most request are going to either send a 200 Ok, or 500 Internal Server error,
    // we might as well intercept any errors here, and straight up return the status.
    let result = handle_packet(request, sender).await;
    if result.is_err() {
        warn!("Error Handling Request, {:?}", result.as_ref().err());
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

async fn get_status(usb_tx: Data<Mutex<DeviceSender>>) -> Result<DaemonStatus> {
    // Unwrap the Mutex Guard..
    let mut guard = usb_tx.lock().await;
    let sender = guard.deref_mut();

    let request = DaemonRequest::GetStatus;

    let result = handle_packet(request, sender).await?;
    return match result {
        DaemonResponse::Status(status) => Ok(status),
        _ => Err(anyhow!("Unexpected Daemon Status Result: {:?}", result)),
    };
}
