use std::{io::Cursor, path::Path};

use flate2::read::GzDecoder;
use tar::Archive;
use warp::hyper::body::Bytes;

use crate::app_error::ServerError;

// We always assume the tarball is gzipped, non-gzipped tarballs will throw an error.
#[tracing::instrument(name = "store_tarball", skip(content))]
pub async fn store_tarball(content: Cursor<Bytes>, target_dir: &Path) -> Result<(), ServerError> {
    let tar = GzDecoder::new(content);
    let mut archive = Archive::new(tar);
    archive.unpack(target_dir)?;
    Ok(())
}
