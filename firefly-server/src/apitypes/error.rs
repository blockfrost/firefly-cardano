use std::fmt::Display;

use aide::OperationOutput;
use axum::{
    response::{IntoResponse, Response},
    Json,
};
use reqwest::StatusCode;
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: impl Into<StatusCode>, message: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            message: message.into(),
        }
    }
    pub fn from_reqwest(err: reqwest::Error) -> Self {
        if let Some(status) = err.status() {
            return Self::new(status, format!("{:#}", err));
        }
        err.into()
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_IMPLEMENTED, message)
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}: {}", self.status, self.message))
    }
}

#[derive(Serialize)]
struct SerializableApiError {
    message: String,
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = SerializableApiError {
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{:#}", value.into()),
        }
    }
}

impl OperationOutput for ApiError {
    type Inner = Self;
}

pub type ApiResult<T, E = ApiError> = std::result::Result<T, E>;

pub trait Context {
    fn context<C>(self, context: C) -> Self
    where
        C: Display;
}

impl<T> Context for ApiResult<T> {
    fn context<C>(self, context: C) -> Self
    where
        C: Display,
    {
        self.map_err(|err| ApiError::new(err.status, format!("{}: {}", context, err.message)))
    }
}

pub trait ToAnyhow {
    type Success;
    fn to_anyhow(self) -> Result<Self::Success, anyhow::Error>;
}

impl<T> ToAnyhow for ApiResult<T> {
    type Success = T;
    fn to_anyhow(self) -> Result<Self::Success, anyhow::Error> {
        self.map_err(|err| anyhow::anyhow!("{}", err.to_string()))
    }
}
