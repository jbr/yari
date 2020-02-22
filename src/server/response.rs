use crate::rpc::{AppendResponse, VoteResponse};
use rocket::{
    http::Status,
    response::{status::Custom, Redirect, Responder},
    Request,
};
use rocket_contrib::json::{Json, JsonError};
use serde_json::json;

#[derive(Debug)]
pub enum Response {
    InternalError(Option<String>),
    ParseError(String, String),
    IoError(String),
    Unavailable,
    Redirect(String),
    AppendResponse(AppendResponse),
    VoteResponse(VoteResponse),
    String(String),
    Success,
}

impl From<url::ParseError> for Response {
    fn from(pe: url::ParseError) -> Self {
        Self::InternalError(Some(format!("parse error: {}", pe.to_string())))
    }
}

impl<'a> From<JsonError<'a>> for Response {
    fn from(je: JsonError<'a>) -> Self {
        match je {
            JsonError::Io(e) => Self::IoError(e.to_string()),
            JsonError::Parse(input, e) => Self::ParseError(e.to_string(), input.to_string()),
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for Response {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::InternalError(Some("poison error".to_owned()))
    }
}

impl From<std::option::NoneError> for Response {
    fn from(_: std::option::NoneError) -> Self {
        Self::InternalError(Some("none error".to_owned()))
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


impl std::ops::Try for Response {
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

impl<'r> Responder<'r> for Response {
    fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
        if let Self::AppendResponse(_) = self {
        } else {
            dbg!(&self);
        }
        match self {
            Self::InternalError(option_string) => Custom(
                Status::InternalServerError,
                json!({"error": option_string.unwrap_or("unknown".into())}).to_string(),
            )
            .respond_to(req),

            Self::ParseError(error, input) => Custom(
                Status::UnprocessableEntity,
                json!({ "error": error, "input": input }).to_string(),
            )
            .respond_to(req),

            Self::IoError(error) => {
                Custom(Status::BadRequest, json!({ "error": error }).to_string()).respond_to(req)
            }

            Self::Unavailable => Custom(
                Status::ServiceUnavailable,
                json!({ "error": "service temporarily unavailable" }).to_string(),
            )
            .respond_to(req),

            Self::Redirect(url) => Redirect::temporary(url).respond_to(req),
            Self::VoteResponse(j) => Json(j).respond_to(req),
            Self::AppendResponse(a) => Json(a).respond_to(req),
            Self::String(s) => s.respond_to(req),
            Self::Success => Json(json!({ "success": true })).respond_to(req),
        }
    }
}
