use actix::{
    Actor, ActorContext, AsyncContext, ContextFutureSpawner, Handler, Message, StreamHandler,
    WrapFuture,
};
use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::dev::ServerHandle;
use actix_web::http::header::ContentType;
use actix_web::middleware::Condition;
use actix_web::web::Data;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use actix_web_actors::ws::{CloseCode, CloseReason};
use anyhow::{anyhow, Result};
use enum_map::EnumMap;
use futures_util::StreamExt;
use include_dir::{include_dir, Dir};
use jsonpath_rust::JsonPathQuery;
use log::{debug, error, info, warn};
use mime_guess::mime::IMAGE_PNG;
use mime_guess::MimeGuess;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Component, PathBuf};
use std::{env, fs};
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, RwLock};

use crate::files::{find_file_in_path, FilePaths};
use crate::PatchEvent;
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DaemonStatus, HttpSettings, Scribble, WebsocketRequest,
    WebsocketResponse,
};
use goxlr_scribbles::get_scribble_png;
use goxlr_types::FaderName;

use crate::primary_worker::{DeviceCommand, DeviceSender};
use crate::servers::server_packet::handle_packet;

const WEB_CONTENT: Dir = include_dir!("./daemon/web-content/");

struct Websocket {
    usb_tx: DeviceSender,
    broadcast_tx: BroadcastSender<PatchEvent>,
}

impl Actor for Websocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let address = ctx.address();
        let mut broadcast_rx = self.broadcast_tx.subscribe();

        // Create a future that simply monitors the global broadcast bus, and pushes any changes
        // out to the WebSocket.
        let future = Box::pin(async move {
            loop {
                if let Ok(event) = broadcast_rx.recv().await {
                    // We've received a message, attempt to trigger the WsMessage Handle..
                    if let Err(error) = address.clone().try_send(WsResponse(WebsocketResponse {
                        id: u64::MAX,
                        data: DaemonResponse::Patch(event.data),
                    })) {
                        error!(
                            "Error Occurred when sending message to websocket: {:?}",
                            error
                        );
                        warn!("Aborting Websocket pushes for this client.");
                        break;
                    }
                }
            }
        });

        let future = future.into_actor(self);
        ctx.spawn(future);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct WsResponse(WebsocketResponse);

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
                match serde_json::from_slice::<WebsocketRequest>(text.as_ref()) {
                    Ok(request) => {
                        let recipient = ctx.address().recipient();
                        let mut usb_tx = self.usb_tx.clone();
                        let future = async move {
                            let request_id = request.id;
                            let result = handle_packet(request.data, &mut usb_tx).await;
                            match result {
                                Ok(resp) => match resp {
                                    DaemonResponse::Ok => {
                                        recipient.do_send(WsResponse(WebsocketResponse {
                                            id: request_id,
                                            data: DaemonResponse::Ok,
                                        }));
                                    }
                                    DaemonResponse::Error(error) => {
                                        recipient.do_send(WsResponse(WebsocketResponse {
                                            id: request_id,
                                            data: DaemonResponse::Error(error),
                                        }));
                                    }
                                    DaemonResponse::Status(status) => {
                                        recipient.do_send(WsResponse(WebsocketResponse {
                                            id: request_id,
                                            data: DaemonResponse::Status(status),
                                        }));
                                    }
                                    DaemonResponse::MicLevel(level) => {
                                        recipient.do_send(WsResponse(WebsocketResponse {
                                            id: request_id,
                                            data: DaemonResponse::MicLevel(level),
                                        }))
                                    }
                                    _ => {}
                                },
                                Err(error) => {
                                    recipient.do_send(WsResponse(WebsocketResponse {
                                        id: request_id,
                                        data: DaemonResponse::Error(error.to_string()),
                                    }));
                                }
                            }
                        };
                        future.into_actor(self).spawn(ctx);
                    }
                    Err(error) => {
                        // Ok, we weren't able to deserialise the request into a proper object, we
                        // now need to confirm whether it was at least valid JSON with a request id
                        warn!("Error Deserialising Request to Object: {}", error);
                        warn!("Original Request: {}", text);

                        debug!("Attempting Low Level request Id Extraction..");
                        let request: serde_json::Result<Value> =
                            serde_json::from_str(text.as_ref());
                        match request {
                            Ok(value) => {
                                if let Some(request_id) = value["id"].as_u64() {
                                    let recipient = ctx.address().recipient();
                                    recipient.do_send(WsResponse(WebsocketResponse {
                                        id: request_id,
                                        data: DaemonResponse::Error(error.to_string()),
                                    }));
                                } else {
                                    warn!("id missing, Cannot continue. Closing connection");
                                    let error = CloseReason {
                                        code: CloseCode::Invalid,
                                        description: Some(String::from(
                                            "Missing or invalid Request ID",
                                        )),
                                    };
                                    ctx.close(Some(error));
                                    ctx.stop();
                                }
                            }
                            Err(error) => {
                                warn!("JSON structure is invalid, closing connection.");
                                let error = CloseReason {
                                    code: CloseCode::Invalid,
                                    description: Some(error.to_string()),
                                };
                                ctx.close(Some(error));
                                ctx.stop();
                            }
                        }
                    }
                }
            }
            Ok(ws::Message::Binary(_bin)) => {
                ctx.close(Some(CloseCode::Unsupported.into()));
                ctx.stop();
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

struct AppData {
    usb_tx: DeviceSender,
    broadcast_tx: BroadcastSender<PatchEvent>,
    file_paths: FilePaths,

    scribble_state: EnumMap<FaderName, ScribbleState>,
}

#[derive(Debug, Default)]
struct ScribbleState {
    scribble_config: Option<Scribble>,
    png_data: Option<Vec<u8>>,
}

pub async fn spawn_http_server(
    usb_tx: DeviceSender,
    handle_tx: Sender<Result<Option<ServerHandle>>>,
    broadcast_tx: tokio::sync::broadcast::Sender<PatchEvent>,
    settings: HttpSettings,
    file_paths: FilePaths,
) {
    // Create the AppData ONCE, outside the closure
    let app_data = Data::new(RwLock::new(AppData {
        broadcast_tx: broadcast_tx.clone(),
        usb_tx: usb_tx.clone(),
        file_paths: file_paths.clone(),
        scribble_state: EnumMap::default(),
    }));

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin_fn(|origin, _req_head| {
                origin.as_bytes().starts_with(b"http://127.0.0.1")
                    || origin.as_bytes().starts_with(b"http://localhost")
            })
            .allow_any_method()
            .allow_any_header()
            .max_age(300);
        App::new()
            .wrap(Condition::new(settings.cors_enabled, cors))
            .app_data(app_data.clone())
            .service(execute_command)
            .service(get_devices)
            .service(get_sample)
            .service(get_scribble)
            .service(get_path)
            .service(websocket)
            .service(upload_firmware)
            .default_service(web::to(default))
    })
    .bind((settings.bind_address.clone(), settings.port));

    if let Err(e) = server {
        // Log the Error Message..
        warn!("Unable to Start HTTP Server: {}", e);

        // Let 'Upstream' know an error has occurred
        let _ = handle_tx.send(Err(anyhow!(e)));

        // Give up :D
        return;
    }

    // Run the server..
    let server = server.unwrap().run();
    info!(
        "Started GoXLR configuration interface at http://{}:{}/",
        settings.bind_address.as_str(),
        settings.port,
    );

    // Let upstream know we're running..
    let _ = handle_tx.send(Ok(Some(server.handle())));

    // Wait for the server to exit with its reason
    let result = server.await;
    if result.is_err() {
        error!("HTTP Server Stopped with Error: {}", result.err().unwrap());
        return;
    }

    info!("HTTP Server Stopped.");
}

#[get("/api/websocket")]
async fn websocket(
    app_data: Data<RwLock<AppData>>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let data = app_data.read().await;

    ws::start(
        Websocket {
            usb_tx: data.usb_tx.clone(),
            broadcast_tx: data.broadcast_tx.clone(),
        },
        &req,
        stream,
    )
}

// So, fun note, according to the actix manual, web::Json uses serde_json to deserialise, good
// news everybody! So do we.. :)
#[post("/api/command")]
async fn execute_command(
    request: web::Json<DaemonRequest>,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    let mut data = app_data.write().await;

    // Errors propagate weirdly in the javascript world, so send all as OK, and handle there.
    match handle_packet(request.0, &mut data.usb_tx).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(error) => HttpResponse::Ok().json(DaemonResponse::Error(error.to_string())),
    }
}

#[get("/api/get-devices")]
async fn get_devices(app_data: Data<RwLock<AppData>>) -> HttpResponse {
    if let Ok(response) = get_status(app_data).await {
        return HttpResponse::Ok().json(&response);
    }
    HttpResponse::InternalServerError().finish()
}

#[get("/api/path")]
async fn get_path(app_data: Data<RwLock<AppData>>, req: HttpRequest) -> HttpResponse {
    let params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    if let Ok(params) = params {
        if let Some(path) = params.get("path") {
            if let Ok(status) = get_status(app_data).await {
                if let Ok(value) = serde_json::to_value(status) {
                    if let Ok(result) = value.path(path) {
                        return HttpResponse::Ok().json(result);
                    } else {
                        warn!("Invalid Path Provided..");
                    }
                } else {
                    warn!("Unable to Parse DaemonStatus..");
                }
            } else {
                warn!("Unable to Fetch Daemon Status..");
            }
        } else {
            warn!("Path Parameter Not Found..");
        }
    } else {
        warn!("Unable to Parse Parameters..");
    }

    HttpResponse::InternalServerError().finish()
}

#[get("/files/scribble/{serial}/{fader}.png")]
async fn get_scribble(
    path: web::Path<(String, FaderName)>,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    let serial = &path.0;
    let fader = path.1;

    let final_width = 128;
    let final_height = 64;

    // Now we need to grab the DaemonResponse to get the layout of the scribble
    let mut data = app_data.write().await;
    let request = DaemonRequest::GetStatus;

    // Pull out the scribble configuration from the Daemon Status
    let scribble = handle_packet(request, &mut data.usb_tx)
        .await
        .ok()
        .and_then(|response| match response {
            DaemonResponse::Status(status) => status
                .mixers
                .get(serial)
                .and_then(|mixer| mixer.fader_status[fader].scribble.clone()),
            _ => None,
        });

    if scribble.is_none() {
        return HttpResponse::NotFound().finish();
    }

    let state = &data.scribble_state[fader];
    if state.scribble_config == scribble && state.png_data.is_some() {
        // If we already have the PNG data, return it.
        if let Some(png_data) = &data.scribble_state[fader].png_data {
            debug!("Returning Cached Scribble Image for {} - {}", serial, fader);
            let mime_type = ContentType(IMAGE_PNG);
            let mut builder = HttpResponse::Ok();
            builder.insert_header(mime_type);
            return builder.body(png_data.clone());
        }
    } else if let Some(scribble) = scribble {
        debug!("Building Scribble Image for {} - {}", serial, fader);
        let scribble_path = data.file_paths.icons.clone();

        let mut icon_path = None;
        if let Some(file) = &scribble.file_name {
            icon_path = Some(scribble_path.join(file));
        }

        let png = get_scribble_png(
            icon_path,
            scribble.bottom_text.clone(),
            scribble.left_text.clone(),
            scribble.inverted,
            final_width,
            final_height,
        );

        if let Ok(png) = png {
            data.scribble_state[fader] = ScribbleState {
                scribble_config: Some(scribble.clone()),
                png_data: Some(png.clone()),
            };

            debug!("Creating Image {}x{}", final_width, final_height);

            let mime_type = ContentType(IMAGE_PNG);
            let mut builder = HttpResponse::Ok();
            builder.insert_header(mime_type);
            return builder.body(png);
        }
    }

    debug!("Unable to Build Image: {} - {}", serial, fader);
    HttpResponse::NotFound().finish()
}

#[get("/files/samples/{sample}")]
async fn get_sample(sample: web::Path<String>, app_data: Data<RwLock<AppData>>) -> HttpResponse {
    // Get the Base Samples Path..
    let sample_path = {
        let data = app_data.read().await;
        data.file_paths.samples.clone()
    };

    let sample = sample.into_inner();
    let path = PathBuf::from(sample);

    if path.components().any(|part| part == Component::ParentDir) {
        // The path provided attempts to leave the samples dir, reject it.
        return HttpResponse::Forbidden().finish();
    }

    debug!("Attempting to Find {:?} in {:?}", path, sample_path);
    let file = find_file_in_path(sample_path, path);
    if let Some(path) = file {
        debug!("Found at {:?}", path);
        let mime_type = MimeGuess::from_path(path.clone()).first_or_octet_stream();
        let mut builder = HttpResponse::Ok();
        builder.insert_header(ContentType(mime_type));
        return builder.body(fs::read(path).unwrap());
    }

    HttpResponse::NotFound().finish()
}

#[post("/firmware-upload/{serial}")]
async fn upload_firmware(
    path: web::Path<String>,
    mut payload: Multipart,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    let serial = path.into_inner();
    let file_path = env::temp_dir().join(format!("{serial}.bin"));

    let mut field = match payload.next().await {
        Some(Ok(field)) => field,
        Some(Err(e)) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error processing multipart: {e}"))
        }
        None => return HttpResponse::BadRequest().body("No file found in request"),
    };

    let mut file = match File::create(&file_path) {
        Ok(f) => f,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("Failed to create file: {e}"))
        }
    };

    while let Some(chunk) = field.next().await {
        match chunk {
            Ok(data) => {
                if let Err(e) = file.write_all(&data) {
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to write file: {e}"));
                }
            }
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .body(format!("Error reading file chunk: {e}"))
            }
        }
    }

    // When we get here, the file has been uploaded successfully...
    let data = app_data.read().await;
    let (tx, rx) = oneshot::channel();

    let _ = data
        .usb_tx
        .send(DeviceCommand::RunFirmwareUpdate(
            serial,
            Some(file_path),
            false,
            tx,
        ))
        .await;
    let result = rx.await;
    match result {
        Ok(_) => HttpResponse::Ok().body(serde_json::to_string(&DaemonResponse::Ok).unwrap()),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error Occurred: {e}")),
    }
}

async fn default(req: HttpRequest) -> HttpResponse {
    let path = if req.path() == "/" || req.path() == "" {
        "/index.html"
    } else {
        req.path()
    };
    let path_part = &path[1..path.len()];
    let file = WEB_CONTENT.get_file(path_part);
    if let Some(file) = file {
        let mime_type = MimeGuess::from_path(path).first_or_octet_stream();
        let mut builder = HttpResponse::Ok();
        builder.insert_header(ContentType(mime_type));
        builder.body(file.contents())
    } else {
        HttpResponse::NotFound().finish()
    }
}

async fn get_status(app_data: Data<RwLock<AppData>>) -> Result<DaemonStatus> {
    let mut data = app_data.write().await;
    let request = DaemonRequest::GetStatus;

    let result = handle_packet(request, &mut data.usb_tx).await?;
    match result {
        DaemonResponse::Status(status) => Ok(status),
        _ => Err(anyhow!("Unexpected Daemon Status Result: {:?}", result)),
    }
}
