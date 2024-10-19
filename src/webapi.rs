use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, ConnectInfo, Query},
    response::IntoResponse,
    routing::any,
    Router,
    extract::State,
};
use axum_extra::TypedHeader;
use futures::{SinkExt as _, StreamExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use serde::Deserialize;

use crate::{config::JPEG_UPDATE_INTERVAL, grid::{self, Flut}};

#[derive(Clone)]
pub struct WebApiContext {
    grids: Arc<[grid::Flut<u32>]>,
}

pub async fn serve(ctx: WebApiContext) {
    // diagnostics
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/imgstream", any(img_stream_ws_handler))
        .with_state(ctx)
        // logging middleware
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );
    
    // run it with hyper
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

#[derive(Debug, Deserialize)]
struct CanvasQuery {
    canvas: u8,
}

async fn img_stream_ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(ctx): State<WebApiContext>,
    Query(CanvasQuery { canvas }): Query<CanvasQuery>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| image_streamer(ctx, canvas, socket, addr))
}

async fn image_streamer(ctx: WebApiContext, canvas: u8, socket: WebSocket, who: SocketAddr) {
    let (mut sender, _) = socket.split();
    
    loop {
        tokio::time::sleep(JPEG_UPDATE_INTERVAL).await;
        let mut buf = Vec::new();
        let jpgbuf = ctx.grids[canvas as usize].read_jpg_buffer().await.clone();
        buf.extend_from_slice(&jpgbuf);
        match sender.send(Message::Binary(buf)).await {
            Ok(_) => (),
            Err(e) => {
                tracing::error!("Error sending image to {who}: {e}");
                return;
            }
        }
    }
}