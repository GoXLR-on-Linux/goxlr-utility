use std::ops::DerefMut;

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
use anyhow::{anyhow, Result};
use futures::lock::Mutex;
use log::{info, warn};
use tokio::sync::oneshot::Sender;

use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus};

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
            .service(execute_command)
            .service(get_devices)
            .service(websocket)
            .service(ResourceFiles::new("/", static_files))
    })
    .bind(("127.0.0.1", port))?
    .run();

    info!(
        "Started GoXLR configuration interface at http://127.0.0.1:{}/",
        port
    );

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

// So, fun note, according to the actix manual, web::Json uses serde_json to deserialise, good
// news everybody! So do we.. :)
#[post("/api/command")]
async fn execute_command(
    request: web::Json<DaemonRequest>,
    usb_mutex: Data<Mutex<DeviceSender>>,
) -> HttpResponse {
    let mut guard = usb_mutex.lock().await;
    let sender = guard.deref_mut();

    // Errors propagate weirdly in the javascript world, so send all as OK, and handle there.
    return match handle_packet(request.0, sender).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(error) => HttpResponse::Ok().json(DaemonResponse::Error(error.to_string())),
    };
}

#[get("/api/get-devices")]
async fn get_devices(usb_mutex: Data<Mutex<DeviceSender>>) -> HttpResponse {
    if let Ok(response) = get_status(usb_mutex).await {
        return HttpResponse::Ok().json(&response);
    }
    HttpResponse::InternalServerError().finish()
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
