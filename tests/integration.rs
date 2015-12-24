extern crate aurelius;
extern crate websocket;

use std::io::prelude::*;
use std::thread;

use aurelius::Server;
use websocket::{Client, Message, Receiver};
use websocket::client::request::Url;

#[test]
fn test_initial_send() {
    let server = Server::new().start();
    let url = Url::parse(&format!("ws://0.0.0.0:{}", server.websocket_port())).unwrap();

    let request = Client::connect(&url).unwrap();
    let response = request.send().unwrap();
    response.validate().unwrap();
    let (_, mut receiver) = response.begin().split();

    server.send_markdown("Hello world!");

    let message: Message = receiver.recv_message().unwrap();
    assert_eq!(String::from_utf8(message.payload.into_owned()).unwrap(), "<p>Hello world!</p>\n");
}

#[test]
fn test_multiple_send() {
    let server = Server::new().start();
    let url = Url::parse(&format!("ws://0.0.0.0:{}", server.websocket_port())).unwrap();

    let request = Client::connect(&url).unwrap();
    let response = request.send().unwrap();
    response.validate().unwrap();
    let (_, mut receiver) = response.begin().split();

    let handle = thread::spawn(move || {
        server.send_markdown("# Hello world!");

        // FIXME: There are some race conditions here...
        thread::sleep_ms(100);

        server.send_markdown("# Goodbye world!");
    });

    let message: Message = receiver.recv_message().unwrap();
    assert_eq!(String::from_utf8(message.payload.into_owned()).unwrap(), "<h1>Hello world!</h1>\n");

    let message: Message = receiver.recv_message().unwrap();
    assert_eq!(String::from_utf8(message.payload.into_owned()).unwrap(), "<h1>Goodbye world!</h1>\n");

    handle.join().unwrap();
}
