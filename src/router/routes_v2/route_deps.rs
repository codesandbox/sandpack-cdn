use std::str::FromStr;
use warp::{http::Uri, Filter};

fn json_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "json" / "deps" / String).map(|dep| {
        let value = format!(
            "https://sandpack-cdn-v2.codesandbox.io/v2/json/deps/{}",
            dep
        );
        warp::redirect(Uri::from_str(&value).unwrap_or(Uri::default()))
    })
}

fn msgpack_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String).map(|dep| {
        let value = format!("https://sandpack-cdn-v2.codesandbox.io/v2/deps/{}", dep);
        warp::redirect(Uri::from_str(&value).unwrap_or(Uri::default()))
    })
}

pub fn deps_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    json_route().or(msgpack_route())
}
