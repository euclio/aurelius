use futures::future;
use tokio::net::lookup_host;

use aurelius::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let addr = lookup_host("localhost:0").await?.next().unwrap();
    let server = Server::bind(&addr).await?;

    server.send("hello world!").await?;

    let () = future::pending().await;

    Ok(())
}
