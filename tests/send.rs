use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

use url::Url;
use websocket::ClientBuilder;
use websocket::ws::dataframe::DataFrame;

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

    let message = client.recv_message().unwrap();
    let html = String::from_utf8(message.take_payload()).unwrap();
    assert_eq!(html.trim(), String::from("<p>Hello, world!</p>"));
}

#[test]
fn no_websockets() {
    let listening = Server::new().start().unwrap();

    // We want to test that there is not a timeout when we send a message to a server that has no
    // websocket connections.. We do this by creating a channel to send data from the If the
    // receiver hasn't received a value within a second, we fail the test.
    let (sender, receiver) = mpsc::channel();

    let _handle = thread::spawn(move || {
        listening.send("This shouldn't hang!").unwrap();
        sender.send(()).unwrap();
    });

    thread::sleep(Duration::from_millis(500));

    if let Err(TryRecvError::Empty) = receiver.try_recv() {
        panic!("test timed out");
    }
}
