extern crate aurelius;
extern crate websocket;
extern crate url;

use websocket::{ClientBuilder, Message};
use url::Url;

use aurelius::Server;

#[test]
fn simple() {
    let mut server = Server::new();
    let handle = server.start();

    let websocket_port = handle.websocket_addr().unwrap().port();

    let url = Url::parse(&format!("ws://localhost:{}", websocket_port)).unwrap();
    let mut client = ClientBuilder::new(url.as_str()).unwrap().connect_insecure().unwrap();

    handle.send("Hello, world!");

    let message: Message = client.recv_message().unwrap();
    let html: String = String::from_utf8(message.payload.to_vec()).unwrap();
    assert_eq!(html.trim(), String::from("<p>Hello, world!</p>"));
}
