use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Error;
use serde::{Deserialize, Serialize};
use slog::{error, Logger};
use warp::http::StatusCode;
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

use crate::device::{Action, Device, State};
use crate::id::{TargetId, DeviceId};

// Request and response types

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum StatusResponse {
    Off,
    Running { target: String },
    Unknown,
}

impl From<State> for StatusResponse {
    fn from(state: State) -> Self {
        match state {
            State::Off => StatusResponse::Off,
            State::Running(target) => StatusResponse::Running {
                target: target.into(),
            },
            State::Unknown => StatusResponse::Unknown,
        }
    }
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    device: String,
    action: String,
}

#[derive(Deserialize)]
struct RunRequest {
    target: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

// Custom Warp rejections

#[derive(Debug)]
struct ActionFailed {
    device: DeviceId,
    error: Error,
}

impl Reject for ActionFailed {}

/// Error handler aware of ActionFailed rejections
async fn handle_error(logger: Logger, err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, error) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not found".to_string())
    } else if let Some(e) = err.find::<ActionFailed>() {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Operation on {} failed: {}", e.device, e.error),
        )
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "HTTP method not allowed".to_string(),
        )
    } else {
        error!(logger, "Unhandled rejection: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error".to_string(),
        )
    };

    let json = warp::reply::json(&ErrorResponse {
        error,
        success: false,
    });
    Ok(warp::reply::with_status(json, code))
}

/// Create a successful action reply
fn action_success(device: &Device, action: Action) -> impl Reply {
    warp::reply::json(&ActionResponse {
        device: device.id().as_string().clone(),
        success: true,
        action: action.to_string(),
    })
}

/// Create a rejection for a failed action
fn action_failure(device: &Device, error: Error) -> warp::Rejection {
    warp::reject::custom(ActionFailed {
        device: device.id().clone(),
        error,
    })
}

/// Serves the Samwise HTTP API
pub async fn serve(logger: Logger, devices: Arc<HashMap<DeviceId, Device>>, addr: SocketAddr) {
    let with_devices = warp::any().map(move || devices.clone());

    // Base for device-scoped endpoints
    let device = warp::path("device")
        .and(warp::path::param())
        .and(with_devices)
        .and_then(
            async move |device_id: String, devices: Arc<HashMap<DeviceId, Device>>| match devices
                .get(&DeviceId::new(device_id))
            {
                Some(device) => Ok(device.clone()),
                None => Err(warp::reject::not_found()),
            },
        );

    let status = device
        .clone()
        .and(warp::path("status"))
        .and(warp::get())
        .map(|device: Device| {
            let response: StatusResponse = device.latest_state().into();
            warp::reply::json(&response)
        });

    let suspend = device
        .clone()
        .and(warp::path("suspend"))
        .and(warp::post())
        .and_then(
            async move |mut device: Device| match device.action(Action::Suspend).await {
                Ok(_) => Ok(action_success(&device, Action::Suspend)),
                Err(error) => Err(action_failure(&device, error)),
            },
        );

    let shutdown = device
        .clone()
        .and(warp::path("shutdown"))
        .and(warp::post())
        .and_then(
            async move |mut device: Device| match device.action(Action::ShutDown).await {
                Ok(_) => Ok(action_success(&device, Action::ShutDown)),
                Err(error) => Err(action_failure(&device, error)),
            },
        );

    let reboot = device
        .clone()
        .and(warp::path("reboot"))
        .and(warp::post())
        .and_then(
            async move |mut device: Device| match device.action(Action::Reboot).await {
                Ok(_) => Ok(action_success(&device, Action::Reboot)),
                Err(error) => Err(action_failure(&device, error)),
            },
        );

    let run = device.and(warp::path("run"))
    .and(warp::post())
    .and(warp::body::content_length_limit(1024)) // Should be more than enough
    .and(warp::body::json::<RunRequest>())
    .and_then(async move |mut device: Device, request: RunRequest| {
        let action = Action::Run(TargetId::new(request.target));
        match device.action(action.clone()).await {
            Ok(_) => Ok(action_success(&device, action)),
            Err(error) => Err(action_failure(&device, error))
        }
    });

    let api = status
        .or(suspend)
        .or(shutdown)
        .or(reboot)
        .or(run)
        .recover(move |err| handle_error(logger.clone(), err));

    warp::serve(api).run(addr).await
}
