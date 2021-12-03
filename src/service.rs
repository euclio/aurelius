use std::convert::Infallible;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

use async_tungstenite::tokio::TokioAdapter;
use async_tungstenite::tungstenite::error::ProtocolError;
use async_tungstenite::tungstenite::handshake::derive_accept_key;
use async_tungstenite::tungstenite::protocol::Role;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures_util::SinkExt;
use handlebars::Handlebars;
use hyper::header::{self, HeaderValue};
use hyper::service::Service;
use hyper::{Body, Request, Response, StatusCode};
use hyper_staticfile::Static;
use include_dir::{include_dir, Dir};
use log::*;
use serde::Serialize;
use tokio::sync::watch::Receiver;
use url::Url;

use crate::Config;

const STATIC_FILES: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");

/// Service that broadcasts received HTML to any listening WebSocket clients.
#[derive(Debug, Clone)]
pub(crate) struct WebsocketBroadcastService {
    pub html_rx: Receiver<String>,
    pub config: Arc<RwLock<Config>>,
}

impl WebsocketBroadcastService {
    async fn handle_request(&mut self, req: Request<Body>) -> Response<Body> {
        if is_websocket_upgrade(&req) {
            let websocket_key = match req.headers().get("Sec-WebSocket-Key") {
                Some(key) => key.as_bytes(),
                None => {
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(ProtocolError::MissingSecWebSocketKey.to_string().into())
                        .unwrap()
                }
            };

            let response = Response::builder()
                .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
                .header(header::CONNECTION, "upgrade")
                .header(header::UPGRADE, "websocket")
                .header(
                    header::SEC_WEBSOCKET_ACCEPT,
                    derive_accept_key(websocket_key),
                )
                .body(Body::empty())
                .unwrap();

            let upgrade = hyper::upgrade::on(req);

            let mut html_rx = self.html_rx.clone();

            // Handle websockets
            tokio::spawn(async move {
                let upgraded = upgrade.await?;

                let mut ws = WebSocketStream::from_raw_socket(
                    TokioAdapter::new(upgraded),
                    Role::Server,
                    None,
                )
                .await;

                while html_rx.changed().await.is_ok() {
                    let html = html_rx.borrow().clone();
                    info!("received new html: {}", html);
                    ws.send(Message::Text(html)).await?;
                }

                let _ = ws.send(Message::Close(None)).await;

                Ok::<_, anyhow::Error>(())
            });

            response
        } else {
            self.serve_file(req).await
        }
    }

    async fn serve_file(&self, req: Request<Body>) -> Response<Body> {
        match req.uri().path() {
            "/" => {
                let config = self.config.read().unwrap();

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

                Response::builder()
                    .status(hyper::StatusCode::OK)
                    .body(Body::from(html))
                    .unwrap()
            }
            path if path.starts_with("/__/") => {
                let path = path.trim_start_matches("/__/");

                let contents = match STATIC_FILES.get_file(path) {
                    Some(file) => file.contents(),
                    None => {
                        error!("{} not found in static files", path);
                        return Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Body::empty())
                            .unwrap();
                    }
                };

                let mime = mime_guess::from_path(path);

                Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header(
                        header::CONTENT_TYPE,
                        mime.first_or_octet_stream().to_string(),
                    )
                    .body(Body::from(contents))
                    .unwrap()
            }
            _ => {
                let dir = self.config.read().unwrap().static_root.clone();
                serve_local_static_file(dir, req).await.unwrap_or_else(|| {
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::empty())
                        .unwrap()
                })
            }
        }
    }
}

async fn serve_local_static_file(
    dir: Option<PathBuf>,
    req: Request<Body>,
) -> Option<Response<Body>> {
    Static::new(dir?).serve(req).await.ok()
}

impl Service<Request<Body>> for WebsocketBroadcastService {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        debug!("incoming request: {:?}", req);

        // Needed to satisfy 'static bound on future.
        let mut service_clone = self.clone();

        Box::pin(async move { Ok(service_clone.handle_request(req).await) })
    }
}

#[derive(Debug, Serialize)]
struct TemplateData<'a> {
    remote_custom_css: &'a [Url],
    local_custom_css: &'a [String],
    highlight_theme: &'a str,
}

fn is_websocket_upgrade<B>(request: &Request<B>) -> bool {
    let headers = request.headers();

    headers.get(header::CONNECTION) == Some(&HeaderValue::from_static("Upgrade"))
        && headers.get(header::UPGRADE) == Some(&HeaderValue::from_static("websocket"))
}
