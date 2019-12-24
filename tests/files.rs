use std::error::Error;
use std::fs;
use std::io::prelude::*;
use std::net::TcpStream;

use reqwest::StatusCode;
use socket2::Socket;

use aurelius::Server;

#[test]
fn not_found() -> Result<(), Box<dyn Error>> {
    let server = Server::bind("localhost:0")?;
    let addr = server.addr();

    let res = reqwest::blocking::get(&format!("http://{}/non-existent", addr))?;

    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[test]
fn static_file_unknown_mime_type() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;
    let file_path = tmp_dir.path().join("file-of-unknown-type");
    fs::write(file_path, ":)")?;

    let mut server = Server::bind("localhost:0")?;
    server.set_static_root(tmp_dir.path());
    let addr = server.addr();

    let res = reqwest::blocking::get(&format!("http://{}/file-of-unknown-type", addr))?;

    assert_eq!(res.status(), StatusCode::OK);

    Ok(())
}

#[test]
fn change_static_root() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;

    fs::write(tmp_dir.path().join("file.txt"), "Lorem ipsum")?;

    let mut server = Server::bind("localhost:0")?;

    let file_url = format!("http://{}/file.txt", server.addr());

    let res = reqwest::blocking::get(&file_url)?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    server.set_static_root(tmp_dir.path());

    let res = reqwest::blocking::get(&file_url)?;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["Content-Type"], "text/plain");
    assert_eq!(res.text()?, "Lorem ipsum");

    Ok(())
}

#[test]
fn change_static_root_to_file() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempfile::tempdir()?;
    let file_path = tmp_dir.path().join("file.txt");

    fs::write(&file_path, "Lorem ipsum")?;

    let mut server = Server::bind("localhost:0")?;

    server.set_static_root(file_path);

    let res = reqwest::blocking::get(&format!("http://{}/", server.addr()))?;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(!res.text()?.contains("Lorem ipsum"));

    let res = reqwest::blocking::get(&format!("http://{}/non-existent", server.addr()))?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[test]
fn static_files() -> Result<(), Box<dyn Error>> {
    let server = Server::bind("localhost:0")?;
    let addr = server.addr();

    let res = reqwest::blocking::get(&format!("http://{}/__/css/styles.css", addr))?;
    assert!(res.status().is_success());
    assert_eq!(res.headers()["Content-Type"], "text/css");
    res.text()?;

    // File in a submodule, make sure that it's included too.
    let res = reqwest::blocking::get(&format!(
        "http://{}/__/vendor/highlight.js/highlight.pack.js",
        addr
    ))?;
    assert!(res.status().is_success());
    assert_eq!(res.headers()["Content-Type"], "application/javascript");
    res.text()?;

    Ok(())
}

/// Tests that the server gracefully handles clients that disconnect in the middle of reading a
/// response. It's a bit hacky (and thus flaky), but the test triggers the desired conditions
/// enough to be valuable.
#[test]
fn partial_read() -> Result<(), Box<dyn Error>> {
    let server = Server::bind("localhost:0")?;

    let mut conn: Socket = TcpStream::connect(server.addr())?.into();

    // Use a very small buffer to make it likely that the socket closes before the write completes.
    conn.set_recv_buffer_size(1)?;

    write!(conn, "GET /__/css/styles.css HTTP/1.1\r\n\r\n")?;
    conn.flush()?;

    let _ = conn.read(&mut [])?;
    drop(conn);

    Ok(())
}

#[test]
fn static_file_not_found() -> Result<(), Box<dyn Error>> {
    let server = Server::bind("localhost:0")?;
    let addr = server.addr();

    let res = reqwest::blocking::get(&format!("http://{}/__/does-not-exist.js", addr))?;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    Ok(())
}
