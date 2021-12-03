use std::error::Error;
use std::fs;

use futures_util::TryStreamExt;
use tempfile::NamedTempFile;

use crate::new_server;

#[tokio::test]
async fn custom_css_url() -> Result<(), Box<dyn Error>> {
    static CSS_URL: &str =
        "http://scholarlymarkdown.com/scholdoc-distribution/css/core/scholmd-core-latest.css";

    let mut server = new_server().await?;

    server.set_custom_css(vec![String::from(CSS_URL)])?;

    let text = reqwest::get(&format!("http://{}", server.addr()))
        .await?
        .text()
        .await?;
    assert!(text.contains(&CSS_URL));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[tokio::test]
async fn custom_css_file() -> Result<(), Box<dyn Error>> {
    let temp_file = NamedTempFile::new()?;
    fs::write(&temp_file, "a { color: #FF0000; }")?;

    let mut server = new_server().await?;

    server.set_custom_css(vec![temp_file.path().display().to_string()])?;

    let text = reqwest::get(&format!("http://{}", server.addr()))
        .await?
        .text()
        .await?;
    assert!(text.contains("<style>a { color: #FF0000; }</style>"));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[tokio::test]
async fn custom_css_file_uri() -> Result<(), Box<dyn Error>> {
    let temp_file = NamedTempFile::new()?;
    fs::write(&temp_file, "a { color: #FF0000; }")?;

    let mut server = new_server().await?;

    server.set_custom_css(vec![format!("file://{}", temp_file.path().display(),)])?;

    let text = reqwest::get(&format!("http://{}", server.addr()))
        .await?
        .text()
        .await?;
    assert!(text.contains("<style>a { color: #FF0000; }</style>"));
    assert!(!text.contains("github-markdown.css"));

    Ok(())
}

#[tokio::test]
async fn custom_css_default() -> Result<(), Box<dyn Error>> {
    let server = new_server().await?;

    let text = reqwest::get(&format!("http://{}", server.addr()))
        .await?
        .text()
        .await?;
    assert!(text.contains("github-markdown.css"));

    Ok(())
}

#[tokio::test]
async fn highlight_theme() -> Result<(), Box<dyn Error>> {
    let mut server = new_server().await?;
    server.set_highlight_theme(String::from("darcula"));

    let text = reqwest::get(&format!("http://{}", server.addr()))
        .await?
        .text()
        .await?;
    assert!(text.contains("darcula.min.css"));

    Ok(())
}

#[cfg(not(windows))]
#[tokio::test]
async fn external_renderer() -> Result<(), Box<dyn Error>> {
    use tokio::process::Command;

    let mut server = new_server().await?;

    server.set_external_renderer(Command::new("cat"));

    let (mut websocket, _) =
        async_tungstenite::tokio::connect_async(format!("ws://{}", server.addr())).await?;

    server.send("Hello, world!").await?;

    let message = websocket.try_next().await?.unwrap();
    assert_eq!(message.to_text()?.trim(), "Hello, world!");

    Ok(())
}
