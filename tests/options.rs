use std::error::Error;
use std::fs;

use tempfile::NamedTempFile;

use aurelius::Server;

#[test]
fn custom_css_url() -> Result<(), Box<dyn Error>> {
    static CSS_URL: &str =
        "http://scholarlymarkdown.com/scholdoc-distribution/css/core/scholmd-core-latest.css";

    let mut server = Server::bind("localhost:0")?;

    server.set_custom_css(vec![String::from(CSS_URL)])?;

    let text = reqwest::blocking::get(&format!("http://{}", server.addr()))?.text()?;
    assert!(text.contains(&CSS_URL));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[test]
fn custom_css_file() -> Result<(), Box<dyn Error>> {
    let temp_file = NamedTempFile::new()?;
    fs::write(&temp_file, "a { color: #FF0000; }")?;

    let mut server = Server::bind("localhost:0")?;

    server.set_custom_css(vec![temp_file.path().display().to_string()])?;

    let text = reqwest::blocking::get(&format!("http://{}", server.addr()))?.text()?;
    assert!(text.contains("<style>a { color: #FF0000; }</style>"));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[test]
fn custom_css_file_uri() -> Result<(), Box<dyn Error>> {
    let temp_file = NamedTempFile::new()?;
    fs::write(&temp_file, "a { color: #FF0000; }")?;

    let mut server = Server::bind("localhost:0")?;

    server.set_custom_css(vec![format!(
        "file://{}",
        temp_file.path().display().to_string()
    )])?;

    let text = reqwest::blocking::get(&format!("http://{}", server.addr()))?.text()?;
    assert!(text.contains("<style>a { color: #FF0000; }</style>"));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[test]
fn custom_css_default() -> Result<(), Box<dyn Error>> {
    let server = Server::bind("localhost:0")?;

    let text = reqwest::blocking::get(&format!("http://{}", server.addr()))?.text()?;
    assert!(text.contains("github-markdown.css"));

    Ok(())
}

#[test]
fn highlight_theme() -> Result<(), Box<dyn Error>> {
    let mut server = Server::bind("localhost:0")?;
    server.set_highlight_theme(String::from("darcula"));

    let text = reqwest::blocking::get(&format!("http://{}", server.addr()))?.text()?;
    assert!(text.contains("darcula.css"));

    Ok(())
}

#[cfg(not(windows))]
#[test]
fn external_renderer() -> Result<(), Box<dyn Error>> {
    use std::process::Command;
    use tungstenite::handshake::client::Request;

    let mut server = Server::bind("localhost:0")?;

    server.set_external_renderer(Command::new("cat"));

    let addr = server.addr();

    let req = Request {
        url: format!("ws://{}", addr).parse()?,
        extra_headers: None,
    };

    let (mut websocket, _) = tungstenite::connect(req)?;

    server.send("Hello, world!")?;

    let message = websocket.read_message()?;
    assert_eq!(message.to_text()?.trim(), "Hello, world!");

    Ok(())
}
