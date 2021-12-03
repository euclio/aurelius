use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::{
    body::Body,
    extract::{
        self,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
        Extension,
    },
    http::{header, HeaderMap, Request, StatusCode, Uri},
    response::{Html, IntoResponse},
};
use handlebars::Handlebars;
use include_dir::{include_dir, Dir};
use serde::Serialize;
use serde::Serializer;
use tokio::sync::watch::Receiver;
use tower::util::ServiceExt;
use tower_http::services::ServeDir;
use tracing::log::*;

use crate::Config;

const STATIC_FILES: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");

pub(crate) async fn serve_asset(extract::Path(path): extract::Path<PathBuf>) -> impl IntoResponse {
    let path = path.strip_prefix("/").unwrap_or(&path);

    let file = match STATIC_FILES.get_file(&path) {
        Some(file) => file,
        None => return Err((StatusCode::NOT_FOUND, "file not found")),
    };

    let mime = mime_guess::from_path(&path);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        mime.first_or_octet_stream().to_string().parse().unwrap(),
    );

    Ok((headers, file.contents()))
}

pub(crate) async fn websocket_handler(
    ws: Option<WebSocketUpgrade>,
    Extension(config): Extension<Arc<RwLock<Config>>>,
    Extension(html_rx): Extension<Receiver<String>>,
) -> impl IntoResponse {
    if let Some(ws) = ws {
        ws.on_upgrade(|ws| async { handle_websocket(ws, html_rx).await })
    } else {
        let config = config.read().unwrap();

        let html = Handlebars::new()
            .render_template(
                include_str!("../templates/markdown_view.html"),
                &TemplateData {
                    remote_custom_css: &config.css_links,
                    local_custom_css: &config.custom_styles,
                    highlight_theme: &config.highlight_theme,
                },
            )
            .unwrap();

        (StatusCode::OK, Html(html)).into_response()
    }
}

async fn handle_websocket(mut socket: WebSocket, mut html_rx: Receiver<String>) {
    while html_rx.changed().await.is_ok() {
        let html = html_rx.borrow().clone();
        info!("received new html: {}", html);
        socket.send(AxumMessage::Text(html)).await.unwrap();
    }

    let _ = socket.send(AxumMessage::Close(None)).await;
}

pub(crate) async fn serve_static_file(
    Extension(config): Extension<Arc<RwLock<Config>>>,
    req: Request<Body>,
) -> impl IntoResponse {
    let static_root = config.read().unwrap().static_root.to_owned();

    let root = match static_root {
        Some(root) => root,
        None => return Err((StatusCode::NOT_FOUND, String::from("file not found"))),
    };

    let service = ServeDir::new(root);

    service
        .oneshot(req)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))
}

#[derive(Debug, Serialize)]
struct TemplateData<'a> {
    #[serde(serialize_with = "serialize_uris_as_strings")]
    remote_custom_css: &'a [Uri],
    local_custom_css: &'a [String],
    highlight_theme: &'a str,
}

fn serialize_uris_as_strings<S>(uris: &[Uri], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_seq(uris.iter().map(Uri::to_string))
}
