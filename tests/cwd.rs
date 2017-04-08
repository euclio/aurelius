extern crate aurelius;
extern crate reqwest;
extern crate url;

use url::Url;
use reqwest::StatusCode;

use aurelius::Server;

#[test]
fn change_working_directory() {
    let mut server = Server::new();
    let mut handle = server.start();

    // Try to find a resource outside of the working directory.
    let http_port = handle.http_addr().unwrap().port();
    let mut resource_url = Url::parse(&format!("http://localhost:{}", http_port)).unwrap();
    resource_url.set_path("/file");

    let response = reqwest::get(resource_url.clone()).unwrap();
    assert_eq!(*response.status(), StatusCode::NotFound);

    // Change to a directory where the file exists
    handle.change_working_directory("tests/assets");

    let response = reqwest::get(resource_url).unwrap();
    assert_eq!(*response.status(), StatusCode::Ok);
}
