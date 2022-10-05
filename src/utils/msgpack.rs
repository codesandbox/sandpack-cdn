extern crate rmp_serde as rmps;

use crate::app_error::ServerError;
use serde::{Deserialize, Serialize};

pub fn serialize_msgpack<T>(value: &T) -> Result<Vec<u8>, ServerError>
where
    T: Serialize,
{
    let mut buf = Vec::new();
    value
        .serialize(&mut rmps::Serializer::new(&mut buf))
        .map_err(|e| ServerError::SerializeError())?;
    Ok(buf)
}

pub fn deserialize_msgpack<'a, T>(input: &'a [u8]) -> Result<T, ServerError>
where
    T: Deserialize<'a>,
{
    rmps::from_slice(input).map_err(|e| ServerError::DeserializeError())
}
