use crate::rpc::{AppendResponse, VoteResponse};
use serde_json::json;
use std::{ops::Try, option::NoneError};
use tide::{http_types::StatusCode, IntoResponse};
use url::ParseError;

#[derive(Debug)]
pub enum Response {
    InternalError(Option<String>),
    Unavailable,
    Redirect(String),
    AppendResponse(AppendResponse),
    VoteResponse(VoteResponse),
    Json(serde_json::Value),
    String(String),
    Success,
}

impl From<ParseError> for Response {
    fn from(pe: ParseError) -> Self {
        Self::InternalError(Some(format!("parse error: {}", pe.to_string())))
    }
}

impl From<NoneError> for Response {
    fn from(_: NoneError) -> Self {
        Self::InternalError(Some("none error".to_owned()))
    }
}

impl From<serde_json::Value> for Response {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

impl From<AppendResponse> for Response {
    fn from(ar: AppendResponse) -> Self {
        Self::AppendResponse(ar)
    }
}

impl From<VoteResponse> for Response {
    fn from(ar: VoteResponse) -> Self {
        Self::VoteResponse(ar)
    }
}

impl From<urlencoding::FromUrlEncodingError> for Response {
    fn from(e: urlencoding::FromUrlEncodingError) -> Self {
        Self::InternalError(Some(format!("urlencoding error: {:?}", e)))
    }
}

impl From<url::Url> for Response {
    fn from(url: url::Url) -> Self {
        Self::Redirect(url.into_string())
    }
}

impl From<String> for Response {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for Response {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl Try for Response {
    type Ok = Response;
    type Error = Response;

    fn into_result(self) -> Result<Response, Response> {
        Ok(self)
    }

    fn from_error(v: Self::Error) -> Self {
        v.into()
    }

    fn from_ok(v: Self::Error) -> Self {
        v.into()
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> tide::Response {
        match self {
            Self::InternalError(option_string) => {
                tide::Response::new(StatusCode::InternalServerError)
                    .body_json(&json!({"error": option_string.unwrap_or("unknown".into())}))
                    .unwrap()
            }

            Self::Unavailable => tide::Response::new(StatusCode::ServiceUnavailable)
                .body_json(&json!({ "error": "service temporarily unavailable" }))
                .unwrap(),

            Self::Redirect(url) => tide::Response::new(StatusCode::TemporaryRedirect)
                .set_header("location".parse().unwrap(), &url),

            Self::VoteResponse(j) => tide::Response::new(StatusCode::Ok).body_json(&j).unwrap(),

            Self::AppendResponse(a) => tide::Response::new(StatusCode::Ok).body_json(&a).unwrap(),

            Self::Json(j) => tide::Response::new(StatusCode::Ok).body_json(&j).unwrap(),

            Self::String(s) => tide::Response::new(StatusCode::Ok).body_string(s),

            Self::Success => tide::Response::new(StatusCode::Ok)
                .body_json(&json!({ "success": true }))
                .unwrap(),
        }
    }
}
