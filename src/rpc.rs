pub mod append;
pub mod client;
pub mod vote;

pub use append::*;
pub use client::*;
pub use vote::*;

//static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);
//use anyhow::Result;
//use openssl::ssl::{SslConnector, SslFiletype, SslMethod, SslVerifyMode};
//use std::convert::TryFrom;
//use std::io::Read;
//use std::sync::Arc;
use surf::{http::Method, Client, Request};

pub fn request(method: Method, url: url::Url) -> Request<impl http_client::HttpClient> {
    let client = Client::new();
    match method {
        Method::GET => client.get(url),
        Method::PUT => client.put(url),
        Method::DELETE => client.delete(url),
        Method::POST => client.post(url),
        _ => panic!("{:?}", method),
    }
}
