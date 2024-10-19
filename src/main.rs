use std::{
    collections::VecDeque,
    fs::{create_dir_all, File},
    io::{self, Error, ErrorKind},
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use chrono::Local;
use debug_print::{debug_eprintln, debug_println};
use flurry::{
    config::{GRID_LENGTH, HOST, IMAGE_SAVE_INTERVAL},
    flutclient::FlutClient,
    grid::{self, Flut},
    COUNTER,
};
use image::{codecs::jpeg::JpegEncoder, GenericImageView, SubImage};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    time::{interval, sleep, timeout, Instant},
};

/// This function logs the current amount of changed pixels to stdout every second
async fn pixel_change_stdout_log() -> () {
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
async fn save_image_frames(grids: Arc<[grid::Flut<u32>]>, duration: Duration) -> io::Result<()> {
    let base_dir = Path::new("./recordings");
    let mut timer = interval(duration);
    create_dir_all(base_dir)?;
    loop {
        timer.tick().await;
        for grid in grids.as_ref() {
            let p = base_dir.join(format!("{}", Local::now().format("%Y-%m-%d %H:%M:%S")));
            debug_println!("timer ticked, grid writing to {:?}", p);
            let mut file_writer = File::create(p)?;

            let encoder = JpegEncoder::new_with_quality(&mut file_writer, 50);
            grid.view(0, 0, grid.width(), grid.height()).to_image();

            let sub_image = SubImage::new(grid, 0, 0, grid.width(), grid.height());
            let image = sub_image.to_image();
            match image.write_with_encoder(encoder) {
                Ok(_) => {}
                Err(err) => eprintln!("{}", err),
            }
        }
    }
}

/// Handle connections made to the socket, keeps a vec of the currently active connections,
/// uses timeout to loop through them and clean them up to stop a memory leak while not throwing
/// everything away
async fn handle_flut(flut_listener: TcpListener, grids: Arc<[grid::Flut<u32>]>) -> () {
    let mut handles = VecDeque::new();
    tokio::spawn(async {
        loop {
            if let Some(handle) = handles.pop_front() {
                match timeout(Duration::from_secs(10), handle).await {
                    Ok(Ok(())) => debug_println!("connection closed ok"),
                    Ok(Err(e)) => debug_eprintln!("connection error {:?}", e),
                    Err(_) => handles.push_back(handle),
                }
            } else {
                sleep(Duration::from_secs(30)).await;
            }
        }
    });
    loop {
        if let Ok((mut socket, _)) = flut_listener.accept().await {
            let grids = grids.clone();
            handles.push_back(tokio::spawn(async move {
                let (reader, writer) = socket.split();
                let mut connection = FlutClient::new(reader, writer, grids);
                let resp = connection.process_socket().await;
                match resp {
                    Ok(()) => Ok(()),
                    Err(err) => Err(err),
                }
            }))
        };
    }
}

#[tokio::main]
#[allow(clippy::needless_return)]
async fn main() {
    println!("created grids");
    let grids: Arc<[Flut<u32>; GRID_LENGTH]> = [grid::Flut::init(800, 600, 0xff_00_ff_ff)].into();

    let Ok(flut_listener) = TcpListener::bind(HOST).await else {
        eprintln!("Was unable to bind to {HOST}, please check if a different process is bound");
        return;
    };
    println!("bound flut listener");

    let handles = vec![
        (tokio::spawn(pixel_change_stdout_log())),
        (tokio::spawn(save_image_frames(grids.clone(), IMAGE_SAVE_INTERVAL))),
        (tokio::spawn(handle_flut(flut_listener, grids.clone()))),
    ];

    for handle in handles {
        println!("joined handle had result {:?}", handle.await);
    }
}
