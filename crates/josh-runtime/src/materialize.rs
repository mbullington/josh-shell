use std::io::{self, Read};

use crate::{ExecutionError, Value};

/// Maximum aggregate bytes retained by one capture or materializing stream stage.
pub const MAX_MATERIALIZED_BYTES: usize = 64 * 1024 * 1024;
/// Maximum values retained by one value capture or `collect` stage.
pub const MAX_MATERIALIZED_ITEMS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterializationLimit {
    Bytes(usize),
    Items(usize),
}

impl std::fmt::Display for MaterializationLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bytes(MAX_MATERIALIZED_BYTES) => {
                write!(formatter, "64 MiB ({MAX_MATERIALIZED_BYTES} bytes)")
            }
            Self::Bytes(limit) => write!(formatter, "{limit} bytes"),
            Self::Items(MAX_MATERIALIZED_ITEMS) => formatter.write_str("1,000,000 items"),
            Self::Items(limit) => write!(formatter, "{limit} items"),
        }
    }
}

#[derive(Debug)]
pub struct BoundedBytes {
    boundary: &'static str,
    limit: usize,
    bytes: Vec<u8>,
}

impl BoundedBytes {
    #[must_use]
    pub const fn new(boundary: &'static str, limit: usize) -> Self {
        Self {
            boundary,
            limit,
            bytes: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), ExecutionError> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(materialization_limit(
                self.boundary,
                MaterializationLimit::Bytes(self.limit),
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|error| {
            ExecutionError::Collect(io::Error::other(format!(
                "cannot allocate {} materialization buffer: {error}",
                self.boundary
            )))
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub fn read_bounded(
    reader: &mut impl Read,
    boundary: &'static str,
    limit: usize,
) -> Result<Vec<u8>, ExecutionError> {
    let mut output = BoundedBytes::new(boundary, limit);
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let remaining = limit.saturating_sub(output.len());
        let read_limit = if remaining < buffer.len() {
            remaining + 1
        } else {
            buffer.len()
        };
        let read = match reader.read(&mut buffer[..read_limit]) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ExecutionError::Collect(error)),
        };
        output.extend_from_slice(&buffer[..read])?;
    }
    Ok(output.into_inner())
}

#[must_use]
pub fn value_materialized_items(value: &Value, limit: usize) -> Option<usize> {
    fn add(total: &mut usize, amount: usize, limit: usize) -> Option<()> {
        *total = total.checked_add(amount)?;
        (*total <= limit).then_some(())
    }

    fn measure_nested(value: &Value, total: &mut usize, limit: usize) -> Option<()> {
        match value {
            Value::Array(values) => {
                add(total, values.len(), limit)?;
                for value in values.iter() {
                    measure_nested(value, total, limit)?;
                }
            }
            Value::Object(object) => {
                add(total, object.len(), limit)?;
                for (_, value) in object.iter() {
                    measure_nested(value, total, limit)?;
                }
            }
            _ => {}
        }
        Some(())
    }

    let mut total = 0;
    measure_nested(value, &mut total, limit)?;
    if total == 0 {
        (limit > 0).then_some(1)
    } else {
        Some(total)
    }
}

#[must_use]
pub fn value_materialized_bytes(value: &Value, limit: usize) -> Option<usize> {
    fn add(total: &mut usize, amount: usize, limit: usize) -> Option<()> {
        *total = total.checked_add(amount)?;
        (*total <= limit).then_some(())
    }

    fn measure(value: &Value, total: &mut usize, limit: usize) -> Option<()> {
        match value {
            Value::Null | Value::Environment | Value::Function(_) => Some(()),
            Value::Bool(_) => add(total, 1, limit),
            Value::Int(_) | Value::Float(_) => add(total, std::mem::size_of::<u64>(), limit),
            Value::String(value) => add(total, value.len(), limit),
            Value::Bytes(value) => add(total, value.len(), limit),
            Value::Array(values) => {
                for value in values.iter() {
                    measure(value, total, limit)?;
                }
                Some(())
            }
            Value::Object(object) => {
                for (key, value) in object.iter() {
                    add(total, key.len(), limit)?;
                    measure(value, total, limit)?;
                }
                Some(())
            }
            Value::Error(error) => {
                add(total, error.kind().len(), limit)?;
                add(total, error.message().len(), limit)
            }
            Value::Status(status) => {
                for outcome in status.outcomes() {
                    add(total, outcome.rendered.len(), limit)?;
                }
                Some(())
            }
        }
    }

    let mut total = 0;
    measure(value, &mut total, limit)?;
    Some(total)
}

#[must_use]
pub const fn materialization_limit(
    boundary: &'static str,
    limit: MaterializationLimit,
) -> ExecutionError {
    ExecutionError::MaterializationLimit { boundary, limit }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, sync::Arc};

    use crate::{ExecutionError, MaterializationLimit, Value};

    use super::{read_bounded, value_materialized_bytes, value_materialized_items};

    #[test]
    fn bounded_read_accepts_exact_limit_and_rejects_one_more_byte() {
        assert_eq!(
            read_bounded(&mut Cursor::new(b"1234"), "test capture", 4).unwrap(),
            b"1234"
        );
        let error = read_bounded(&mut Cursor::new(b"12345"), "test capture", 4)
            .expect_err("limit plus one");
        assert!(error.to_string().contains("streaming `filter`/`take`"));
        assert!(matches!(
            error,
            ExecutionError::MaterializationLimit {
                boundary: "test capture",
                limit: MaterializationLimit::Bytes(4)
            }
        ));
    }

    #[test]
    fn value_measurement_stops_at_the_same_exact_byte_boundary() {
        let value = Value::Array(Arc::new(vec![
            Value::String(Arc::from("12")),
            Value::Bytes(Arc::from(&b"34"[..])),
        ]));
        assert_eq!(value_materialized_bytes(&value, 4), Some(4));
        assert_eq!(value_materialized_bytes(&value, 3), None);
        assert_eq!(value_materialized_items(&value, 2), Some(2));
        assert_eq!(value_materialized_items(&value, 1), None);
    }
}
