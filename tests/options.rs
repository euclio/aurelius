extern crate aurelius;
extern crate reqwest;

use std::io::prelude::*;

use aurelius::Server;

#[test]
fn custom_css() {
    let css_url = "http://scholarlymarkdown.com/scholdoc-distribution/css/core/scholmd-core-latest.css";

    let listening = Server::new()
        .css(css_url)
        .start()
        .unwrap();
    let url = format!("http://localhost:{}", listening.http_addr().unwrap().port());
    let mut response = String::new();
    reqwest::get(&url).unwrap().read_to_string(&mut response).unwrap();
    assert!(response.contains(&css_url));
}

#[test]
fn highlight_theme() {
    let listening = Server::new()
        .highlight_theme("darcula")
        .start()
        .unwrap();
    let url = format!("http://localhost:{}", listening.http_addr().unwrap().port());
    let mut response = String::new();
    reqwest::get(&url).unwrap().read_to_string(&mut response).unwrap();
    let link = "/vendor/highlight.js/styles/darcula.css";

    assert!(response.contains(link));
}
