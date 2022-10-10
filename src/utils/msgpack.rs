extern crate rmp_serde as rmps;

use crate::app_error::ServerError;
use serde::Serialize;

pub fn serialize_msgpack<T>(value: &T) -> Result<Vec<u8>, ServerError>
where
    T: Serialize,
{
    let mut buf = Vec::new();
    let serializer = rmps::Serializer::new(&mut buf);
    value
        .serialize(&mut serializer.with_struct_map())
        .map_err(|_e| ServerError::SerializeError())?;
    Ok(buf)
}
