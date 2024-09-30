use std::{error::Error, path::PathBuf};

use rocket::{get, http::Status, post, routes, serde::json::Json, Config, State};
use serde::Deserialize;
use utils::WebcamManager;
use v4l::Device;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

mod utils;

// Fetch all the available v4l cameras in the system
#[get("/available_cameras")]
async fn get_available_cameras(state: &State<AppState>) -> Result<Json<Vec<PathBuf>>, (Status, &'static str)> {
    Ok(Json(state.available_camera_paths.clone()))
}

#[derive(Deserialize)]
struct CameraRTCRequest {
    path: String,
    offer: RTCSessionDescription
}

#[post("/cameras/start", data = "<request>")]
async fn get_video_feed(request: Json<CameraRTCRequest>, state: &State<AppState>) -> Result<Json<RTCSessionDescription>, Status> {
    let request = request.into_inner();
    let webcam_manager = &state.webcam_manager;
    let local_description = webcam_manager.add_client(request.path, request.offer).await
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(local_description))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut available_camera_paths: Vec<PathBuf> = h264_webcam_stream::list_devices();
    available_camera_paths.retain(|path| {
        let mut device: Device = h264_webcam_stream::get_device(&path).unwrap();

        // If can't query or create a stream, then it can't be displayed.
        device.query_controls().is_ok() && h264_webcam_stream::stream(&mut device, 1).is_ok()
    });

    // Create a Rocket instance with the default configuration.
    rocket::build()
        // This is arbitrary and can be changed at any time through a config file or it can be left hardcoded.
        .configure(Config::figment().merge(("port", 3600)))
        .mount("/stream", routes![get_available_cameras, get_video_feed])
        .manage(AppState {
            webcam_manager: WebcamManager::new().unwrap(),
            available_camera_paths
        })
        .launch().await?;

    Ok(())
}

struct AppState {
    webcam_manager: WebcamManager,
    available_camera_paths: Vec<PathBuf>
}