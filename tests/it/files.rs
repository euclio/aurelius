use std::error::Error;

use reqwest::StatusCode;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpSocket;

use crate::new_server;

#[tokio::test]
async fn not_found() -> Result<(), Box<dyn Error>> {
    let server = new_server().await?;
    let addr = server.addr();

    let res = reqwest::get(&format!("http://{}/non-existent", addr)).await?;

    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn static_file_unknown_mime_type() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;
    let file_path = tmp_dir.path().join("file-of-unknown-type");
    fs::write(file_path, ":)").await?;

    let mut server = new_server().await?;
    server.set_static_root(tmp_dir.path());
    let addr = server.addr();

    let res = reqwest::get(&format!("http://{}/file-of-unknown-type", addr)).await?;

    assert_eq!(res.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn change_static_root() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;

    fs::write(tmp_dir.path().join("file.txt"), "Lorem ipsum").await?;

    let mut server = new_server().await?;

    let file_url = format!("http://{}/file.txt", server.addr());

    let res = reqwest::get(&file_url).await?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    server.set_static_root(tmp_dir.path());

    let res = reqwest::get(&file_url).await?;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["Content-Type"], "text/plain");
    assert_eq!(res.text().await?, "Lorem ipsum");

    Ok(())
}

#[tokio::test]
async fn change_static_root_to_file() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;
    let file_path = tmp_dir.path().join("file.txt");

    fs::write(&file_path, "Lorem ipsum").await?;

    let mut server = new_server().await?;

    server.set_static_root(file_path);

    let res = reqwest::get(&format!("http://{}/", server.addr())).await?;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(!res.text().await?.contains("Lorem ipsum"));

    let res = reqwest::get(&format!("http://{}/non-existent", server.addr())).await?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn static_files() -> Result<(), Box<dyn Error>> {
    let mut server = new_server().await?;
    server.set_static_root("static");
    let addr = server.addr();

    let res = reqwest::get(&format!("http://{}/__/css/styles.css", addr)).await?;
    assert!(res.status().is_success());
    assert_eq!(res.headers()["Content-Type"], "text/css");
    res.text().await?;

    // File in a submodule, make sure that it's included too.
    let res = reqwest::get(&format!(
        "http://{}/__/vendor/highlight.js/build/highlight.min.js",
        addr
    ))
    .await?;
    assert!(res.status().is_success());
    assert_eq!(res.headers()["Content-Type"], "application/javascript");
    res.text().await?;

    Ok(())
}

/// Tests that the server gracefully handles clients that disconnect in the middle of reading a
/// response. It's a bit hacky (and thus flaky), but the test triggers the desired conditions
/// enough to be valuable.
#[tokio::test]
async fn partial_read() -> Result<(), Box<dyn Error>> {
    let server = new_server().await?;

    let socket = TcpSocket::new_v6()?;

    // Use a very small buffer to make it likely that the socket closes before the write completes.
    socket.set_recv_buffer_size(1)?;
    let mut conn = socket.connect(server.addr()).await?;

    let partial_req = "GET /__/css/styles.css HTTP/1.1\r\n\r\n";
    conn.write(partial_req.as_bytes()).await?;
    conn.flush().await?;

    let _ = conn.read(&mut []).await?;
    drop(conn);

    Ok(())
}

#[tokio::test]
async fn static_file_not_found() -> Result<(), Box<dyn Error>> {
    let server = new_server().await?;
    let addr = server.addr();

    let res = reqwest::get(&format!("http://{}/__/does-not-exist.js", addr)).await?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}
