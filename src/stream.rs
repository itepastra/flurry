use std::time::Duration;

use axum::http::{self, HeaderMap, HeaderValue};
use axum_streams::StreamingFormat;
use futures::{stream, Stream, StreamExt};
use rand::{distributions::Standard, thread_rng, Rng};

use crate::config;

pub(crate) struct Multipart {
    first: bool,
    boundary: Vec<u8>,
    headers: HeaderMap,
}

impl Multipart {
    pub(crate) fn new(boundary_length: usize, headers: HeaderMap) -> Self {
        let boundary = thread_rng()
            .sample_iter(Standard)
            .filter(|c| match c {
                32..127 | 128..=255 => true,
                0..32 | 127 => false,
            })
            .take(boundary_length)
            .collect();

        Multipart {
            first: false,
            boundary,
            headers,
        }
    }
}

impl<T> StreamingFormat<T> for Multipart
where
    T: Send + Sync + IntoIterator<Item = u8> + 'static,
{
    fn to_bytes_stream<'a, 'b>(
        &'a self,
        stream: futures::stream::BoxStream<'b, Result<T, axum::Error>>,
        _options: &'a axum_streams::StreamBodyAsOptions,
    ) -> futures::stream::BoxStream<'b, Result<axum::body::Bytes, axum::Error>> {
        fn write_multipart_frame<T>(
            obj: T,
            boundary: Vec<u8>,
            headers: HeaderMap,
            first: bool,
        ) -> Result<Vec<u8>, axum::Error>
        where
            T: IntoIterator<Item = u8>,
        {
            let mut frame_vec = Vec::new();
            if first {
                frame_vec.extend_from_slice(b"--");
            } else {
                frame_vec.extend_from_slice(b"\r\n--");
            }
            frame_vec.extend(boundary);
            frame_vec.extend_from_slice(b"\r\n");
            for (header_name, header_value) in headers {
                match header_name {
                    Some(header) => {
                        frame_vec.extend_from_slice(header.as_str().as_bytes());
                        frame_vec.extend_from_slice(b": ");
                        frame_vec.extend_from_slice(header_value.as_bytes());
                        frame_vec.extend_from_slice(b"\r\n");
                    }
                    None => todo!(),
                }
            }
            frame_vec.extend_from_slice(b"\r\n");
            frame_vec.extend(obj);

            Ok(frame_vec)
        }

        let boundary = self.boundary.clone();
        let headers = self.headers.clone();
        let first = self.first;

        Box::pin({
            stream.map(move |obj_res| match obj_res {
                Err(e) => Err(e),
                Ok(obj) => {
                    let picture_framed =
                        write_multipart_frame(obj, boundary.clone(), headers.clone(), first);

                    picture_framed.map(axum::body::Bytes::from)
                }
            })
        })
    }

    fn http_response_headers(
        &self,
        _options: &axum_streams::StreamBodyAsOptions,
    ) -> Option<axum::http::HeaderMap> {
        let mut header_map = HeaderMap::new();
        let mut multipart: Vec<u8> = "multipart/x-mixed-replace; boundary=".into();
        multipart.extend_from_slice(&self.boundary);

        header_map.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_bytes(multipart.as_slice()).unwrap(),
        );

        Some(header_map)
    }
}
