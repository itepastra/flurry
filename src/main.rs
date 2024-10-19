use std::{
    convert::Infallible,
    fs::{create_dir_all, File},
    io::{self, Write as _},
    path::Path,
    sync::Arc,
    time::Duration,
};

use chrono::Local;
use flurry::{
    config::{GRID_LENGTH, HOST, IMAGE_SAVE_INTERVAL, JPEG_UPDATE_INTERVAL},
    flutclient::FlutClient,
    grid::{self, Flut},
    COUNTER,
};
use tokio::{
    net::TcpListener,
    time::interval
};
type Never = Infallible;

/// This function logs the current amount of changed pixels to stdout every second
async fn pixel_change_stdout_log() -> io::Result<Never> {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        println!("{cnt} pixels were changed");
    }
}

/// This function starts a timer that saves the current grid state every `duration`.
/// These images may then be used for moderation or timelapses
///
/// # Errors
///
/// This function will return an error if it is unable to create or write to the file for the image
async fn save_image_frames(grids: Arc<[grid::Flut<u32>]>, duration: Duration) -> io::Result<Never> {
    let base_dir = Path::new("./recordings");
    let mut timer = interval(duration);
    create_dir_all(base_dir)?;
    loop {
        timer.tick().await;
        for grid in grids.as_ref() {
            let p = base_dir.join(format!("{}", Local::now().format("%Y-%m-%d_%H-%M-%S.jpg")));
            let mut file_writer = File::create(p)?;

            file_writer.write_all(&grid.read_jpg_buffer().await)?;
        }
    }
}

/// Handle connections made to the socket, keeps a vec of the currently active connections,
/// uses timeout to loop through them and clean them up to stop a memory leak while not throwing
/// everything away
async fn handle_flut(flut_listener: TcpListener, grids: Arc<[grid::Flut<u32>]>) -> io::Result<Never> {
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

async fn jpeg_update_loop(grids: Arc<[Flut<u32>]>) -> io::Result<Never> {
    let mut interval = interval(JPEG_UPDATE_INTERVAL);
    loop {
        interval.tick().await;
        for grid in grids.as_ref() {
            grid.update_jpg_buffer().await;
        }
    }
}

#[tokio::main]
#[allow(clippy::needless_return)]
async fn main() {
    let grids: Arc<[Flut<u32>; GRID_LENGTH]> = [grid::Flut::init(800, 600, 0xff_00_ff_ff)].into();
    println!("created grids");

    let Ok(flut_listener) = TcpListener::bind(HOST).await else {
        eprintln!("Was unable to bind to {HOST}, please check if a different process is bound");
        return;
    };
    println!("bound flut listener");

    let handles = vec![
        (tokio::spawn(pixel_change_stdout_log())),
        (tokio::spawn(save_image_frames(grids.clone(), IMAGE_SAVE_INTERVAL))),
        (tokio::spawn(handle_flut(flut_listener, grids.clone()))),
        (tokio::spawn(jpeg_update_loop(grids.clone())))
    ];

    for handle in handles {
        println!("joined handle had result {:?}", handle.await);
    }
}