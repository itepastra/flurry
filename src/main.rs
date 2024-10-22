use std::{
    fs::{create_dir_all, File},
    io::Write as _,
    path::Path,
    process::exit,
    sync::Arc,
    time::Duration,
};

use chrono::Local;
use flurry::{
    config::{GRID_LENGTH, HOST, IMAGE_SAVE_INTERVAL, JPEG_UPDATE_INTERVAL},
    flutclient::FlutClient,
    grid::{self, Flut},
    webapi::WebApiContext,
    AsyncResult, COUNTER,
};
use futures::never::Never;
use tokio::{net::TcpListener, time::interval, try_join};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

/// This function logs the current amount of changed pixels to stdout every second
async fn pixel_change_stdout_log() -> AsyncResult<Never> {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        tracing::info!("{cnt} pixels changed");
    }
}

/// This function starts a timer that saves the current grid state every `duration`.
/// These images may then be used for moderation or timelapses
///
/// # Errors
///
/// This function will return an error if it is unable to create or write to the file for the image
async fn save_image_frames(
    grids: Arc<[grid::Flut<u32>; GRID_LENGTH]>,
    duration: Duration,
) -> AsyncResult<Never> {
    let base_dir = Path::new("./recordings");
    let mut timer = interval(duration);
    create_dir_all(base_dir)?;
    loop {
        timer.tick().await;
        for grid in grids.as_ref() {
            let p = base_dir.join(format!("{}", Local::now().format("%Y-%m-%d_%H-%M-%S.jpg")));
            let mut file_writer = File::create(p)?;

            file_writer.write_all(&grid.read_jpg_buffer())?;
        }
    }
}

/// Handle connections made to the socket, keeps a vec of the currently active connections,
/// uses timeout to loop through them and clean them up to stop a memory leak while not throwing
/// everything away
async fn handle_flut(
    flut_listener: TcpListener,
    grids: Arc<[grid::Flut<u32>]>,
) -> AsyncResult<Never> {
    let mut handles = Vec::new();
    loop {
        let (mut socket, _) = flut_listener.accept().await?;
        let grids = grids.clone();
        handles.push(tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut connection = FlutClient::new(reader, writer, grids);
            let resp = connection.process_socket().await;
            match resp {
                Ok(()) => Ok(()),
                Err(err) => Err(err),
            }
        }))
    }
}

async fn jpeg_update_loop(grids: Arc<[Flut<u32>]>) -> AsyncResult<Never> {
    let mut interval = interval(JPEG_UPDATE_INTERVAL);
    loop {
        interval.tick().await;
        for grid in grids.as_ref() {
            grid.update_jpg_buffer();
        }
    }
}

#[tokio::main]
#[allow(clippy::needless_return)]
async fn main() {
    // diagnostics
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let grids: Arc<[Flut<u32>; GRID_LENGTH]> = [grid::Flut::init(800, 600, 0xff_00_ff_ff)].into();
    tracing::trace!("created grids");

    let Ok(flut_listener) = TcpListener::bind(HOST).await else {
        tracing::error!(
            "Was unable to bind to {HOST}, please check if a different process is bound"
        );
        exit(1);
    };
    tracing::info!("Started TCP listener on {HOST}");

    let pixel_logger = tokio::spawn(pixel_change_stdout_log());
    let snapshots = tokio::spawn(save_image_frames(grids.clone(), IMAGE_SAVE_INTERVAL));
    let pixelflut_server = tokio::spawn(handle_flut(flut_listener, grids.clone()));
    let jpeg_update_loop = tokio::spawn(jpeg_update_loop(grids.clone()));
    let website = tokio::spawn(flurry::webapi::serve(WebApiContext {
        grids: grids.clone(),
    }));

    let res = try_join! {
        pixel_logger,
        snapshots,
        pixelflut_server,
        jpeg_update_loop,
        website,
    };
    tracing::error!("something went wrong {:?}", res);
}
