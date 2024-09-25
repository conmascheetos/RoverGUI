use std::error::Error;

use nokhwa::utils::{ApiBackend, CameraInfo};
use rocket::{get, http::Status, routes, serde::json::Json, Config};

#[get("/available_cameras")]
async fn get_available_cameras() -> Result<Json<Vec<CameraInfo>>, (Status, &'static str)> {
    // Fetch available cameras using nokhwa.
    let cameras = nokhwa::query(ApiBackend::Auto)
        .map_err(|_| (Status::InternalServerError, "Failed to Fetch Cameras"))?;

    // Using the "serialize" feature of nokhwa, so CameraInfo derives serialize automatically.
    Ok(Json(cameras))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a Rocket instance with the default configuration.
    rocket::build()
        // This is arbitrary and can be changed at any time through a config file or it can be left hardcoded.
        .configure(Config::figment().merge(("port", 3600)))
        .mount("/stream", routes![get_available_cameras])
        .launch().await?;

    Ok(())
}
