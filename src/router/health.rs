use warp::{Filter, Rejection, Reply};

pub async fn health_route_handler() -> Result<impl Reply, Rejection> {
    Ok("ok")
}

pub fn health_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("health")
        .and(warp::get())
        .and_then(health_route_handler)
}
