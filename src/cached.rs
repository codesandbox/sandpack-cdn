// There's a whole blog article about this: https://fasterthanli.me/articles/request-coalescing-in-async-rust
// Don't need to reinvent the wheel here, but we do iterate further on this
use parking_lot::Mutex;
use std::future::Future;
use std::{
    pin::Pin,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use tokio::sync::broadcast;
use tracing::debug;

pub type BoxFut<'a, O> = Pin<Box<dyn Future<Output = O> + Send + 'a>>;

#[derive(Debug, Clone, thiserror::Error)]
#[error("stringified error: {inner}")]
pub struct CachedError {
    inner: String,
}

impl CachedError {
    pub fn new<E: std::fmt::Display>(e: E) -> Self {
        Self {
            inner: e.to_string(),
        }
    }
}

impl From<broadcast::error::RecvError> for CachedError {
    fn from(e: broadcast::error::RecvError) -> Self {
        CachedError::new(e)
    }
}

#[derive(Clone)]
pub struct Cached<T>
where
    T: Clone + Send + Sync + 'static,
{
    inner: Arc<Mutex<CachedInner<T>>>,
    refresh_interval: Duration,
}

type LastFetched<T> = Option<(Instant, T)>;

struct CachedInner<T>
where
    T: Clone + Send + Sync + 'static,
{
    last_fetched: LastFetched<T>,
    inflight: Option<Weak<broadcast::Sender<Result<T, CachedError>>>>,
}

impl<T> Default for CachedInner<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            last_fetched: None,
            inflight: None,
        }
    }
}

impl<T> Cached<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(refresh_interval: Duration) -> Self {
        Self {
            inner: Default::default(),
            refresh_interval,
        }
    }

    pub async fn get_cached<F, E>(&self, f: F) -> Result<T, CachedError>
    where
        F: FnOnce(Option<T>) -> BoxFut<'static, Result<T, E>> + Send + 'static,
        E: std::fmt::Display + 'static,
    {
        let mut rx = {
            // only sync code in this block
            let mut inner = self.inner.lock();

            if let Some((fetched_at, value)) = inner.last_fetched.as_ref() {
                if fetched_at.elapsed() < self.refresh_interval {
                    return Ok(value.clone());
                } else {
                    debug!("stale, let's refresh");
                }
            }

            let last_fetched = inner.last_fetched.clone().map(|v| v.1);
            
            if let Some(inflight) = inner.inflight.as_ref().and_then(Weak::upgrade) {
                inflight.subscribe()
            } else {
                // there isn't, let's fetch
                let (tx, rx) = broadcast::channel::<Result<T, CachedError>>(1);
                // let's reference-count a single `Sender`:
                let tx = Arc::new(tx);
                // and only store a weak reference in our state:
                inner.inflight = Some(Arc::downgrade(&tx));
                let inner = self.inner.clone();

                // call the closure first, so we don't send _it_ across threads,
                // just the Future it returns
                let fut = f(last_fetched);

                tokio::spawn(async move {
                    let res = fut.await;

                    {
                        // only sync code in this block
                        let mut inner = inner.lock();
                        inner.inflight = None;

                        match res {
                            Ok(value) => {
                                inner.last_fetched.replace((Instant::now(), value.clone()));
                                let _ = tx.send(Ok(value));
                            }
                            Err(e) => {
                                let _ = tx.send(Err(CachedError {
                                    inner: e.to_string(),
                                }));
                            }
                        };
                    }
                });
                rx
            }
        };

        // if we reached here, we're waiting for an in-flight request (we weren't able to serve from cache)
        let received = rx.recv().await??;
        Ok(received)
    }
}
