extern crate aurelius;
extern crate hyper;
extern crate url;

use std::default::Default;
use std::io::prelude::*;

use hyper::Client;
use url::Url;

use aurelius::{Config, Server};

fn get_basic_response(server: &Server) -> String {
    let http_addr = server.http_addr().unwrap();

    let url = Url::parse(&format!("http://localhost:{}", http_addr.port())).unwrap();

    let mut res = Client::new().get(url).send().unwrap();
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();
    body
}

#[test]
fn custom_css() {
    let url = "http://scholarlymarkdown.com/scholdoc-distribution/css/core/scholmd-core-latest.css";

    let server =
        Server::new_with_config(Config { custom_css: String::from(url), ..Default::default() });
    let response = get_basic_response(&server);

    assert!(response.contains(url));
}


#[test]
fn highlight_theme() {
    let server = Server::new_with_config(Config {
        highlight_theme: String::from("darcula"),
        ..Default::default()
    });
    let response = get_basic_response(&server);
    let link = "/vendor/highlight.js/styles/darcula.css";

    assert!(response.contains(link));
}
