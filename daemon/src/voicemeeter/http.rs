use crate::voicemeeter::config::VmProfile;
use crate::voicemeeter::{ApiRequest, request};
use actix_web::http::header::{CacheControl, CacheDirective, ContentType};
use actix_web::{HttpResponse, get, post, web};
use anyhow::Result;
use goxlr_ipc::DaemonResponse;
use lazy_static::lazy_static;

/// The upstream UI is shipped pre-built, so rather than modifying it we inject our own script
/// into its index page. Updating the web content from upstream needs no changes here.
const INDEX: &str = include_str!("../../web-content/index.html");
const UI_SCRIPT: &str = include_str!("ui/voicemeeter.js");
const UI_STYLE: &str = include_str!("ui/voicemeeter.css");

const INJECT: &str = concat!(
    "<link rel=\"stylesheet\" href=\"/voicemeeter/voicemeeter.css\">",
    "<script defer src=\"/voicemeeter/voicemeeter.js\"></script>",
);

lazy_static! {
    static ref INJECTED_INDEX: String = match INDEX.find("</head>") {
        Some(position) => format!("{}{}{}", &INDEX[..position], INJECT, &INDEX[position..]),
        None => format!("{INDEX}{INJECT}"),
    };
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(index)
        .service(index_html)
        .service(ui_file)
        .service(get_state)
        .service(set_config)
        .service(save)
        .service(reload);
}

#[get("/")]
async fn index() -> HttpResponse {
    index_response()
}

#[get("/index.html")]
async fn index_html() -> HttpResponse {
    index_response()
}

fn index_response() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(ContentType::html())
        .body(INJECTED_INDEX.as_str())
}

#[get("/voicemeeter/{file}")]
async fn ui_file(file: web::Path<String>) -> HttpResponse {
    let (content, mime) = match file.as_str() {
        "voicemeeter.js" => (UI_SCRIPT, "text/javascript; charset=utf-8"),
        "voicemeeter.css" => (UI_STYLE, "text/css; charset=utf-8"),
        _ => return HttpResponse::NotFound().finish(),
    };
    HttpResponse::Ok()
        .insert_header(("Content-Type", mime))
        .insert_header(CacheControl(vec![CacheDirective::NoCache]))
        .body(content)
}

#[get("/api/voicemeeter/state")]
async fn get_state() -> HttpResponse {
    match request(ApiRequest::GetState).await {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(e) => HttpResponse::Ok().json(DaemonResponse::Error(e.to_string())),
    }
}

#[post("/api/voicemeeter/config/{serial}")]
async fn set_config(serial: web::Path<String>, config: web::Json<VmProfile>) -> HttpResponse {
    let serial = serial.into_inner();
    let config = config.into_inner();
    respond(request(|tx| ApiRequest::SetConfig(serial, config, tx)).await)
}

#[post("/api/voicemeeter/save/{serial}")]
async fn save(serial: web::Path<String>) -> HttpResponse {
    let serial = serial.into_inner();
    respond(request(|tx| ApiRequest::Save(serial, tx)).await)
}

#[post("/api/voicemeeter/reload/{serial}")]
async fn reload(serial: web::Path<String>) -> HttpResponse {
    let serial = serial.into_inner();
    respond(request(|tx| ApiRequest::Reload(serial, tx)).await)
}

// Same convention as /api/command, errors are returned as an OK response.
fn respond(result: Result<Result<()>>) -> HttpResponse {
    match result.and_then(|r| r) {
        Ok(()) => HttpResponse::Ok().json(DaemonResponse::Ok),
        Err(e) => HttpResponse::Ok().json(DaemonResponse::Error(e.to_string())),
    }
}
