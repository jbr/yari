use async_std::{
    io::{BufReader, Read as AsyncRead},
    stream::Stream,
    task::{ready, Context, Poll},
};
use std::{io, pin::Pin};
use tide::{http::headers::CACHE_CONTROL, http::headers::CONTENT_TYPE, Body};

pin_project_lite::pin_project! {
    /// An SSE protocol encoder.
    #[derive(Debug, Clone)]
    pub struct Encoder<S> {
        buf: Option<Vec<u8>>,
        #[pin]
        receiver: S,
        cursor: usize,
    }
}

impl<E, S> AsyncRead for Encoder<S>
where
    E: Event,
    S: Unpin + Stream<Item = E>,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // Request a new buffer if we don't have one yet.
        if let None = self.buf {
            log::trace!("> waiting for event");
            self.buf = match ready!(Pin::new(&mut self.receiver).poll_next(cx)) {
                Some(event) => {
                    let encoded = encode(&event);
                    log::trace!("> Received a new event with len {}", encoded.len());
                    Some(encoded)
                }
                None => {
                    log::trace!("> Encoder done reading");
                    return Poll::Ready(Ok(0));
                }
            };
        };

        // Write the current buffer to completion.
        let local_buf = self.buf.as_mut().unwrap();
        let local_len = local_buf.len();
        let max = buf.len().min(local_buf.len());
        buf[..max].clone_from_slice(&local_buf[..max]);

        self.cursor += max;

        // Reset values if we're done reading.
        if self.cursor == local_len {
            self.buf = None;
            self.cursor = 0;
        };

        // Return bytes read.
        Poll::Ready(Ok(max))
    }
}

pub trait Event {
    fn name(&self) -> &str;
    fn data(&self) -> &[u8];
    fn id(&self) -> Option<&str>;
}

pub trait EventStream: Sized + Unpin + Send + Sync {
    fn into_encoder(self) -> Encoder<Self>;
    fn into_response(self) -> tide::Response;
}

fn encode<E: Event>(event: &E) -> Vec<u8> {
    log::trace!("> encoding event ");

    let mut data = String::new();
    data.push_str(&format!("event:{}\n", event.name()));
    if let Some(id) = event.id() {
        data.push_str(&format!("id:{}\n", id));
    }
    data.push_str("data:");
    let mut data = data.into_bytes();
    data.extend_from_slice(&event.data());
    data.push(b'\n');
    data.push(b'\n');
    data
}

impl<S, E> Into<tide::Response> for Encoder<S>
where
    S: Sync + Send + Unpin + Stream<Item = E> + 'static,
    E: Event,
{
    fn into(self) -> tide::Response {
        tide::Response::builder(200)
            .body(Body::from_reader(BufReader::new(self), None))
            .header(CACHE_CONTROL, "no-cache")
            .header(CONTENT_TYPE, "text/event-stream")
            .into()
    }
}

impl<E: Event, S: Send + Sync + Unpin + Stream<Item = E> + 'static> EventStream for S {
    fn into_encoder(self) -> Encoder<Self> {
        Encoder {
            receiver: self,
            buf: None,
            cursor: 0,
        }
    }

    fn into_response(self) -> tide::Response {
        self.into_encoder().into_response()
    }
}
