use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, Query, State},
    http::{self, HeaderMap, HeaderName, HeaderValue},
    response::IntoResponse,
    routing::any,
    Router,
};
use axum_extra::TypedHeader;
use axum_streams::StreamBodyAs;
use futures::{never::Never, stream::repeat_with, Stream};
use serde::Deserialize;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::{config::JPEG_UPDATE_INTERVAL, grid, stream::Multipart, AsyncResult};

#[derive(Clone)]
pub struct WebApiContext {
    pub grids: Arc<[grid::Flut<u32>]>,
}

pub async fn serve(ctx: WebApiContext) -> AsyncResult<Never> {
    let app = Router::new()
        .route("/imgstream", any(image_stream))
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

    Err("Web api exited".into())
}

#[derive(Debug, Deserialize)]
struct CanvasQuery {
    canvas: u8,
}

fn make_image_stream(
    ctx: WebApiContext,
    canvas: u8,
) -> impl Stream<Item = Result<Vec<u8>, axum::Error>> {
    use tokio_stream::StreamExt;
    let mut buf = Vec::new();
    repeat_with(move || {
        buf.clear();
        buf.extend_from_slice(&(&ctx.grids[canvas as usize]).read_jpg_buffer());
        Ok(buf.clone())
    })
    .throttle(JPEG_UPDATE_INTERVAL)
}

async fn image_stream(
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
    tracing::info!("`{user_agent}` at {addr} connected.");
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("image/jpeg"),
    );

    StreamBodyAs::new(Multipart::new(10, headers), make_image_stream(ctx, canvas))
}
