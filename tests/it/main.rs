use tokio::net::lookup_host;

use aurelius::{MarkdownRenderer, Server};

mod files;
mod options;

async fn new_server() -> anyhow::Result<Server<MarkdownRenderer>> {
    let addr = lookup_host("localhost:0").await?.next().unwrap();
    Ok(Server::bind(&addr, MarkdownRenderer::new()).await?)
}
