use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::dev::ServerHandle;
use actix_web::http::Method;
use actix_web::http::header::{COOKIE, ContentType, HeaderValue, ORIGIN, REFERER, SET_COOKIE};
use actix_web::middleware::Condition;
use actix_web::web::Data;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, get, post, web};
use actix_ws::{AggregatedMessage, CloseCode, CloseReason, Session};
use anyhow::{Result, anyhow};
use enum_map::EnumMap;
use futures_util::StreamExt;
use include_dir::{Dir, include_dir};
use jsonpath_rust::JsonPath;
use log::{debug, error, info, warn};
use mime_guess::MimeGuess;
use mime_guess::mime::IMAGE_PNG;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Component, PathBuf};
use std::{env, fs};
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::oneshot::Sender;
use tokio::sync::{RwLock, oneshot};

use crate::PatchEvent;
use crate::files::{FilePaths, find_file_in_path};
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DaemonStatus, HttpSettings, Scribble, WebsocketRequest,
    WebsocketResponse,
};
use goxlr_scribbles::get_scribble_png;
use goxlr_types::FaderName;

use crate::primary_worker::{DeviceCommand, DeviceSender};
use crate::servers::server_packet::handle_packet;

const WEB_CONTENT: Dir = include_dir!("./daemon/web-content/");
const AUTH_COOKIE_NAME: &str = "goxlr_http_auth";

struct AppData {
    usb_tx: DeviceSender,
    broadcast_tx: BroadcastSender<PatchEvent>,
    file_paths: FilePaths,
    http_auth_token: Option<String>,

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
        http_auth_token: settings.auth_token.clone(),
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
            .service(websocket)
            .service(execute_command)
            .service(get_devices)
            .service(get_sample)
            .service(get_scribble)
            .service(get_path)
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

    // Let upstream know we're running...
    let _ = handle_tx.send(Ok(Some(server.handle())));

    // Wait for the server to exit with its reason
    let result = server.await;
    if result.is_err() {
        error!("HTTP Server Stopped with Error: {}", result.err().unwrap());
        return;
    }

    info!("HTTP Server Stopped.");
}

#[derive(Serialize)]
struct WsResponse(WebsocketResponse);

#[get("/api/websocket")]
async fn websocket(
    app_data: Data<RwLock<AppData>>,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    if !is_request_authorized(&app_data, &req).await {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let (response, mut session, msg_stream) = actix_ws::handle(&req, body)?;

    let data = app_data.read().await;
    let mut usb_tx = data.usb_tx.clone();
    let mut broadcast_rx = data.broadcast_tx.subscribe();

    // Spawn the handler (this is now where we do stuff)
    actix_web::rt::spawn(async move {
        let mut msg_stream = msg_stream.aggregate_continuations();

        let close_reason = loop {
            tokio::select! {
                Ok(patch) = broadcast_rx.recv() => {
                    let message = WsResponse(WebsocketResponse {
                        id: u64::MAX,
                        data: DaemonResponse::Patch(patch.data),
                    });
                    if let Err(e) = send_response(message, &mut session).await {
                        break e;
                    }
                }
                Some(Ok(msg)) = msg_stream.next() => {
                    match msg {
                        AggregatedMessage::Ping(msg) => {
                            if let Err(e) = session.pong(&msg).await {
                                error!("Failed to send Pong: {}", e);
                                break Some(CloseReason {
                                    code: CloseCode::Error,
                                    description: Some(format!("Failed to Send Pong: {}", e)),
                                });
                            };
                        }
                        AggregatedMessage::Text(msg) => {
                            match serde_json::from_slice::<WebsocketRequest>(msg.as_ref()) {
                                Ok(request) => {
                                    let request_id = request.id;
                                    let result = handle_packet(request.data, &mut usb_tx).await;
                                    let response = match result {
                                        Ok(resp) => {
                                            match resp {
                                                DaemonResponse::Ok => {
                                                    WsResponse(WebsocketResponse {
                                                        id: request_id,
                                                        data: DaemonResponse::Ok,
                                                    })
                                                }
                                                DaemonResponse::Error(error) => {
                                                    WsResponse(WebsocketResponse {
                                                        id: request_id,
                                                        data: DaemonResponse::Error(error),
                                                    })
                                                }
                                                DaemonResponse::Status(status) => {
                                                    WsResponse(WebsocketResponse {
                                                        id: request_id,
                                                        data: DaemonResponse::Status(status),
                                                    })
                                                }
                                                DaemonResponse::MicLevel(level) => {
                                                    WsResponse(WebsocketResponse {
                                                        id: request_id,
                                                        data: DaemonResponse::MicLevel(level),
                                                    })
                                                }
                                                _ => {
                                                    // This should never fucking happen
                                                    break Some(CloseReason {
                                                        code: CloseCode::Abnormal,
                                                        description: Some("Unexpected Message Type".to_string()),
                                                    });
                                                }
                                            }
                                        },
                                        Err(error) => {
                                            WsResponse(WebsocketResponse {
                                                id: request_id,
                                                data: DaemonResponse::Error(error.to_string()),
                                            })
                                        }
                                    };
                                    if let Err(e) = send_response(response, &mut session).await {
                                        break e;
                                    }
                                }
                                Err(error) => {
                                    // Ok, we weren't able to deserialise the request into a proper object, we
                                    // now need to confirm whether it was at least valid JSON with a request id
                                    warn!("Error Deserialising Request to Object: {}", error);
                                    warn!("Original Request: {}", msg);

                                    debug!("Attempting Low Level request Id Extraction..");
                                    let request: serde_json::Result<Value> = serde_json::from_str(msg.as_ref());
                                    match request {
                                        Ok(value) => {
                                            if let Some(request_id) = value["id"].as_u64() {
                                                let response = WsResponse(WebsocketResponse {
                                                    id: request_id,
                                                    data: DaemonResponse::Error(error.to_string()),
                                                });
                                                if let Err(e) = send_response(response, &mut session).await {
                                                    break e;
                                                }
                                            } else {
                                                warn!("id missing, Cannot continue. Closing connection");
                                                let error = CloseReason {
                                                    code: CloseCode::Invalid,
                                                    description: Some(String::from(
                                                        "Missing or invalid Request ID",
                                                    )),
                                                };
                                                break Some(error);
                                            }
                                        }
                                        Err(error) => {
                                            warn!("JSON structure is invalid, closing connection.");
                                            let error = CloseReason {
                                                code: CloseCode::Invalid,
                                                description: Some(error.to_string()),
                                            };
                                            break Some(error);
                                        }
                                    }
                                }
                            }
                        }
                        AggregatedMessage::Binary(_) => {
                            error!("Received Binary Message, aborting!");
                            break Some(CloseReason {
                                code: CloseCode::Unsupported,
                                description: Some("Binary is not Supported".to_string()),
                            });
                        }
                        AggregatedMessage::Pong(_) => {}
                        AggregatedMessage::Close(reason) => {
                            break reason;
                        }
                    }
                }
                else => {
                    break None;
                }
            }
        };

        let _ = session.close(close_reason).await;
    });

    Ok(response)
}

/// Serialises and sends a WsResponse to a Session
async fn send_response(res: WsResponse, session: &mut Session) -> Result<(), Option<CloseReason>> {
    match serde_json::to_string(&res) {
        Ok(text) => {
            if let Err(e) = session.text(text).await {
                error!("Failed to send response: {}", e);
                return Err(Some(CloseReason {
                    code: CloseCode::Error,
                    description: Some(e.to_string()),
                }));
            }
        }
        Err(e) => {
            error!("Failed to serialize response: {}", e);
            return Err(Some(CloseReason {
                code: CloseCode::Error,
                description: Some(format!("Serialization Error: {}", e)),
            }));
        }
    }
    Ok(())
}

// So, fun note, according to the actix manual, web::Json uses serde_json to deserialise, good
// news everybody! So do we.. :)
#[post("/api/command")]
async fn execute_command(
    req: HttpRequest,
    request: web::Json<DaemonRequest>,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

    let mut data = app_data.write().await;

    // Errors propagate weirdly in the javascript world, so send all as OK, and handle there.
    match handle_packet(request.0, &mut data.usb_tx).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(error) => HttpResponse::Ok().json(DaemonResponse::Error(error.to_string())),
    }
}

#[get("/api/get-devices")]
async fn get_devices(req: HttpRequest, app_data: Data<RwLock<AppData>>) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

    if let Ok(response) = get_status(app_data).await {
        return HttpResponse::Ok().json(&response);
    }
    HttpResponse::InternalServerError().finish()
}

#[get("/api/path")]
async fn get_path(app_data: Data<RwLock<AppData>>, req: HttpRequest) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

    let params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
    if let Ok(params) = params {
        if let Some(path) = params.get("path") {
            if let Ok(status) = get_status(app_data).await {
                if let Ok(value) = serde_json::to_value(status) {
                    if let Ok(result) = value.query(path) {
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
    req: HttpRequest,
    path: web::Path<(String, FaderName)>,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

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
async fn get_sample(
    req: HttpRequest,
    sample: web::Path<String>,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

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
        return match fs::read(path.clone()) {
            Ok(content) => builder.body(content),
            Err(error) => {
                warn!("Unable to read sample file {:?}: {}", path, error);
                HttpResponse::InternalServerError().finish()
            }
        };
    }

    HttpResponse::NotFound().finish()
}

#[post("/firmware-upload/{serial}")]
async fn upload_firmware(
    req: HttpRequest,
    path: web::Path<String>,
    mut payload: Multipart,
    app_data: Data<RwLock<AppData>>,
) -> HttpResponse {
    if !is_request_authorized(&app_data, &req).await {
        return HttpResponse::Unauthorized().finish();
    }

    let serial = path.into_inner();
    let file_path = env::temp_dir().join(format!("{serial}.bin"));

    let mut field = match payload.next().await {
        Some(Ok(field)) => field,
        Some(Err(e)) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error processing multipart: {e}"));
        }
        None => return HttpResponse::BadRequest().body("No file found in request"),
    };

    let mut file = match File::create(&file_path) {
        Ok(f) => f,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("Failed to create file: {e}"));
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
                    .body(format!("Error reading file chunk: {e}"));
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
        Ok(_) => HttpResponse::Ok().json(DaemonResponse::Ok),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error Occurred: {e}")),
    }
}

async fn default(req: HttpRequest, app_data: Data<RwLock<AppData>>) -> HttpResponse {
    let path = if req.path() == "/" || req.path() == "" {
        "/index.html"
    } else {
        req.path()
    };
    let path_part = &path[1..path.len()];
    let file = WEB_CONTENT.get_file(path_part);
    if let Some(file) = file {
        if let Some(cookie) = build_auth_cookie_from_query(&app_data, &req).await {
            let redirect_path = if req.path().is_empty() {
                "/"
            } else {
                req.path()
            };
            return HttpResponse::Found()
                .insert_header((SET_COOKIE, cookie))
                .insert_header(("Location", redirect_path))
                .finish();
        }

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

async fn is_request_authorized(app_data: &Data<RwLock<AppData>>, req: &HttpRequest) -> bool {
    let data = app_data.read().await;
    is_token_authorized(req, data.http_auth_token.as_deref())
}

fn is_token_authorized(req: &HttpRequest, expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };

    if let Some(auth) = req.headers().get("Authorization")
        && let Ok(auth) = auth.to_str()
        && let Some(token) = auth.strip_prefix("Bearer ")
        && token == expected
    {
        return true;
    }

    if has_auth_cookie(req, expected) && is_cookie_auth_origin_allowed(req) {
        return true;
    }

    // Query-string bearer tokens are a compromise for websocket clients, which
    // cannot reliably set Authorization headers from browser WebSocket APIs.
    // Do not allow them on ordinary HTTP endpoints where they are more likely
    // to leak through logs, browser history, or referrers.
    if req.path() == "/api/websocket" {
        let params = web::Query::<HashMap<String, String>>::from_query(req.query_string());
        if let Ok(params) = params
            && let Some(token) = params.get("token")
            && token == expected
        {
            return true;
        }
    }

    false
}

async fn build_auth_cookie_from_query(
    app_data: &Data<RwLock<AppData>>,
    req: &HttpRequest,
) -> Option<HeaderValue> {
    let data = app_data.read().await;
    let expected = data.http_auth_token.as_deref()?;
    let params = web::Query::<HashMap<String, String>>::from_query(req.query_string()).ok()?;
    let token = params.get("token")?;

    if token != expected || !is_cookie_value_safe(token) {
        return None;
    }

    HeaderValue::from_str(&format!(
        "{AUTH_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict"
    ))
    .ok()
}

fn has_auth_cookie(req: &HttpRequest, expected: &str) -> bool {
    req.headers()
        .get_all(COOKIE)
        .filter_map(|value| value.to_str().ok())
        .flat_map(|header| header.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .any(|(name, value)| name == AUTH_COOKIE_NAME && value == expected)
}

fn is_cookie_auth_origin_allowed(req: &HttpRequest) -> bool {
    let request_origin = request_origin(req);

    if let Some(origin) = req
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        return origin == request_origin;
    }

    if let Some(referer) = req
        .headers()
        .get(REFERER)
        .and_then(|value| value.to_str().ok())
    {
        return referer == request_origin || referer.starts_with(&format!("{request_origin}/"));
    }

    if req.path() == "/api/websocket" {
        return false;
    }

    matches!(*req.method(), Method::GET | Method::HEAD)
}

fn request_origin(req: &HttpRequest) -> String {
    let connection_info = req.connection_info();
    format!("{}://{}", connection_info.scheme(), connection_info.host())
}

fn is_cookie_value_safe(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte == 0x21
            || (0x23..=0x2b).contains(&byte)
            || (0x2d..=0x3a).contains(&byte)
            || (0x3c..=0x5b).contains(&byte)
            || (0x5d..=0x7e).contains(&byte)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn auth_allows_bearer_header() {
        let req = TestRequest::default()
            .insert_header(("Authorization", "Bearer expected-token"))
            .to_http_request();

        assert!(is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_allows_session_cookie() {
        let req = TestRequest::default()
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_allows_same_origin_session_cookie_on_post() {
        let req = TestRequest::post()
            .uri("/api/command")
            .insert_header(("Host", "127.0.0.1:14564"))
            .insert_header((ORIGIN, "http://127.0.0.1:14564"))
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_rejects_cross_origin_session_cookie_on_post() {
        let req = TestRequest::post()
            .uri("/api/command")
            .insert_header(("Host", "127.0.0.1:14564"))
            .insert_header((ORIGIN, "http://127.0.0.1:3000"))
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(!is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_rejects_cross_origin_websocket_cookie() {
        let req = TestRequest::with_uri("/api/websocket")
            .insert_header(("Host", "127.0.0.1:14564"))
            .insert_header((ORIGIN, "http://127.0.0.1:3000"))
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(!is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_allows_same_origin_websocket_cookie() {
        let req = TestRequest::with_uri("/api/websocket")
            .insert_header(("Host", "127.0.0.1:14564"))
            .insert_header((ORIGIN, "http://127.0.0.1:14564"))
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_rejects_websocket_cookie_without_origin() {
        let req = TestRequest::with_uri("/api/websocket")
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=expected-token")))
            .to_http_request();

        assert!(!is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn auth_allows_query_token_for_websocket_only() {
        let websocket_req =
            TestRequest::with_uri("/api/websocket?token=expected-token").to_http_request();
        let command_req =
            TestRequest::with_uri("/api/command?token=expected-token").to_http_request();

        assert!(is_token_authorized(&websocket_req, Some("expected-token")));
        assert!(!is_token_authorized(&command_req, Some("expected-token")));
    }

    #[test]
    fn auth_rejects_wrong_cookie() {
        let req = TestRequest::default()
            .insert_header((COOKIE, format!("{AUTH_COOKIE_NAME}=wrong-token")))
            .to_http_request();

        assert!(!is_token_authorized(&req, Some("expected-token")));
    }

    #[test]
    fn cookie_values_must_be_header_safe() {
        assert!(is_cookie_value_safe("abc-123_~"));
        assert!(!is_cookie_value_safe("has spaces"));
        assert!(!is_cookie_value_safe("has;semicolon"));
    }
}
