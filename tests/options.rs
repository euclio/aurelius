extern crate aurelius;
extern crate reqwest;

use std::default::Default;
use std::io::prelude::*;

use aurelius::{Config, Server};

#[test]
fn custom_css() {
    let css_url = "http://scholarlymarkdown.com/scholdoc-distribution/css/core/scholmd-core-latest.css";

    let mut server = Server::new_with_config(Config {
        custom_css: String::from(css_url),
        ..Default::default()
    });
    let handle = server.start();
    let url = format!("http://localhost:{}", handle.http_addr().unwrap().port());
    let mut response = String::new();
    reqwest::get(&url).unwrap().read_to_string(&mut response).unwrap();
    assert!(response.contains(&css_url));
}

#[test]
fn highlight_theme() {
    let mut server = Server::new_with_config(Config {
        highlight_theme: String::from("darcula"),
        ..Default::default()
    });
    let handle = server.start();
    let url = format!("http://localhost:{}", handle.http_addr().unwrap().port());
    let mut response = String::new();
    reqwest::get(&url).unwrap().read_to_string(&mut response).unwrap();
    let link = "/vendor/highlight.js/styles/darcula.css";

    assert!(response.contains(link));
}
