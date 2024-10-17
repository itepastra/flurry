#![feature(test)]
#![feature(sync_unsafe_cell)]
#![feature(if_let_guard)]

use std::{
    fs::{create_dir_all, File},
    io::{self, Error, ErrorKind},
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use chrono::Local;
use flurry::{
    config::{GRID_LENGTH, HOST},
    flutclient::FlutClient,
    grid::{self, Flut},
    COUNTER,
};
use image::{codecs::jpeg::JpegEncoder, GenericImageView, SubImage};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    time::{interval, Instant},
};

extern crate test;

async fn listen_handle() -> io::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        println!("{cnt} pixels were changed");
    }
}

async fn save_image_frames(grids: Arc<[grid::Flut<u32>]>) -> io::Result<()> {
    let base_dir = Path::new("./recordings");
    let mut timer = interval(Duration::from_secs(5));
    create_dir_all(base_dir)?;
    loop {
        timer.tick().await;
        for grid in grids.as_ref() {
            let p = base_dir.join(format!("{}", Local::now().format("%Y-%m-%d %H:%M:%S")));
            println!("timer ticked, grid writing to {:?}", p);
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

async fn handle_flut(flut_listener: TcpListener, grids: Arc<[grid::Flut<u32>]>) -> io::Result<()> {
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
        }));
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
        // log the amount of changed pixels each second
        (tokio::spawn(listen_handle())),
        // save frames every 5 seconds
        (tokio::spawn(save_image_frames(grids.clone()))),
        // accept and handle flut connections
        (tokio::spawn(handle_flut(flut_listener, grids.clone()))),
    ];

    for handle in handles {
        println!("joined handle had result {:?}", handle.await);
    }
}

#[cfg(test)]
mod tests {}
