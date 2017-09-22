extern crate aurelius;
extern crate websocket;
extern crate url;

use websocket::{ClientBuilder, Message};
use url::Url;

use aurelius::Server;

#[test]
fn simple() {
    let listening = Server::new().start().unwrap();

    let websocket_port = listening.websocket_addr().unwrap().port();

    let url = Url::parse(&format!("ws://localhost:{}", websocket_port)).unwrap();
    let mut client = ClientBuilder::new(url.as_str())
        .unwrap()
        .connect_insecure()
        .unwrap();

    listening.send("Hello, world!").unwrap();

    let message: Message = client.recv_message().unwrap();
    let html: String = String::from_utf8(message.payload.to_vec()).unwrap();
    assert_eq!(html.trim(), String::from("<p>Hello, world!</p>"));
}
