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

pub async fn request(
    method: surf::http::Method,
    url: surf::url::Url,
) -> surf::Result<surf::Response> {
    surf::Client::new()
        .send(surf::Request::new(method, url))
        .await
}
