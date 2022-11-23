use crate::app_error::{SendableResult, ServerError, AppResult};

use super::types::changes::{ChangeEvent, Event};
use futures_core::{Future, Stream};
use futures_util::{ready, FutureExt, StreamExt, TryStreamExt};
use parking_lot::Mutex;
use reqwest::{Client, Method, Response, StatusCode};
use std::{
    collections::HashMap,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;

/// The max timeout value for longpoll/continous HTTP requests
/// that CouchDB supports (see [1]).
///
/// [1]: https://docs.couchdb.org/en/stable/api/database/changes.html
const COUCH_MAX_TIMEOUT: usize = 60000;

/// The stream for the `_changes` endpoint.
///
/// This is returned from [Database::changes].
pub struct ChangesStream {
    last_seq: Option<serde_json::Value>,
    client: Arc<Client>,
    state: ChangesStreamState,
    params: Arc<Mutex<HashMap<String, String>>>,
}

enum ChangesStreamState {
    Idle,
    Requesting(Pin<Box<dyn Future<Output = SendableResult<Response>>>>),
    Reading(Pin<Box<dyn Stream<Item = io::Result<String>>>>),
}

impl ChangesStream {
    /// Create a new changes stream.
    pub fn new(last_seq: Option<serde_json::Value>) -> Self {
        let client = Arc::new(Client::new());
        let mut params = HashMap::new();
        params.insert("feed".to_string(), "continuous".to_string());
        params.insert("timeout".to_string(), "0".to_string());
        params.insert("include_docs".to_string(), "true".to_string());
        params.insert("timeout".to_string(), COUCH_MAX_TIMEOUT.to_string());
        Self {
            client,
            params: Arc::new(Mutex::new(params)),
            state: ChangesStreamState::Idle,
            last_seq,
        }
    }

    /// Get the last retrieved seq.
    pub fn last_seq(&self) -> &Option<serde_json::Value> {
        &self.last_seq
    }
}

async fn get_changes(
    client: Arc<Client>,
    params: Arc<Mutex<HashMap<String, String>>>,
) -> SendableResult<Response> {
    let inner_params: HashMap<String, String> = params.lock().clone();
    let request = client
        .request(Method::GET, "https://replicate.npmjs.com/registry/_changes")
        .query(&inner_params);
    let res = request.send().await;
    Ok(res)
}

impl Stream for ChangesStream {
    type Item = SendableResult<ChangeEvent>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            self.state = match self.state {
                ChangesStreamState::Idle => {
                    let mut params = self.params.clone();
                    if let Some(seq) = &self.last_seq {
                        params.lock().insert("since".to_string(), seq.to_string());
                    }
                    let fut = get_changes(self.client.clone(), params);
                    ChangesStreamState::Requesting(Box::pin(fut))
                }
                ChangesStreamState::Requesting(ref mut fut) => match ready!(fut.poll_unpin(cx)) {
                    Err(err) => return Poll::Ready(Some(Err(err))),
                    Ok(res) => match res.status().is_success() {
                        true => {
                            let stream = res
                                .bytes_stream()
                                .map_err(|err| io::Error::new(io::ErrorKind::Other, err));
                            let reader = StreamReader::new(stream);
                            let lines = Box::pin(LinesStream::new(reader.lines()));
                            ChangesStreamState::Reading(lines)
                        }
                        false => {
                            let err = Err(ServerError::RequestErrorStatus {
                                status_code: res.status().into(),
                            });

                            return Poll::Ready(Some(err));
                        }
                    },
                },
                ChangesStreamState::Reading(ref mut lines) => {
                    let line = ready!(lines.poll_next_unpin(cx));
                    match line {
                        None => ChangesStreamState::Idle,
                        Some(Err(err)) => {
                            let inner = err
                                .get_ref()
                                .and_then(|err| err.downcast_ref::<reqwest::Error>());
                            match inner {
                                Some(reqwest_err) if reqwest_err.is_timeout() => {
                                    ChangesStreamState::Idle
                                }
                                Some(reqwest_err) => {
                                    let err = ServerError::RequestErrorStatus {
                                        status_code: reqwest_err
                                            .status()
                                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
                                            .into(),
                                    };
                                    return Poll::Ready(Some(Err(err)));
                                }
                                _ => {
                                    let err = ServerError::RequestErrorStatus { status_code: 500 };
                                    return Poll::Ready(Some(Err(err)));
                                }
                            }
                        }
                        Some(Ok(line)) if line.is_empty() => continue,
                        Some(Ok(line)) => match serde_json::from_str::<Event>(&line) {
                            Ok(Event::Change(event)) => {
                                self.last_seq = Some(event.seq.clone());
                                return Poll::Ready(Some(Ok(event)));
                            }
                            Ok(Event::Finished(event)) => {
                                self.last_seq = Some(event.last_seq.clone());
                                ChangesStreamState::Idle
                            }
                            Err(e) => {
                                return Poll::Ready(Some(Err(e.into())));
                            }
                        },
                    }
                }
            }
        }
    }
}
