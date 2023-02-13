use std::str::FromStr;
use warp::{http::Uri, Filter};

pub fn mod_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "mod" / String).map(|dep| {
        let value = format!("https://sandpack-cdn-v2.codesandbox.io/v2/mod/{}", dep);
        warp::redirect(Uri::from_str(&value).unwrap_or(Uri::default()))
    })
}
