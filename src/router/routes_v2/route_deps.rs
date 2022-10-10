use std::collections::HashSet;

use node_semver::Range;
use warp::{Filter, Rejection, Reply};

use crate::app_error::ServerError;
use crate::npm::dep_tree_builder::{DepRequest, DepTreeBuilder};
use crate::package::process::parse_package_specifier_no_validation;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;
use super::super::utils::decode_req_part;
use crate::npm::package_data::PackageDataFetcher;

fn parse_query(query: String) -> Result<HashSet<DepRequest>, ServerError> {
    let parts = query.split(";");
    let mut dep_requests: HashSet<DepRequest> = HashSet::new();
    for part in parts {
        let (name, version) = parse_package_specifier_no_validation(part)?;
        let parsed_range = Range::parse(version)?;
        dep_requests.insert(DepRequest::new(name, parsed_range));
    }
    Ok(dep_requests)
}

pub async fn get_deps_reply(
    path: String,
    pkg_fetcher: PackageDataFetcher,
) -> Result<CustomReply, ServerError> {
    let decoded_query = decode_req_part(path.as_str())?;
    let dep_requests = parse_query(decoded_query)?;

    let mut tree_builder = DepTreeBuilder::new(pkg_fetcher);
    tree_builder.push(dep_requests).await?;
    let reply = CustomReply::json(&tree_builder.resolutions)?;
    Ok(reply)
}

pub async fn deps_route_handler(
    path: String,
    pkg_fetcher: PackageDataFetcher,
) -> Result<impl Reply, Rejection> {
    match get_deps_reply(path, pkg_fetcher).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn deps_route(
    pkg_fetcher: PackageDataFetcher,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "deps" / String)
        .and(warp::get())
        .and(with_data(pkg_fetcher))
        .and_then(deps_route_handler)
}
