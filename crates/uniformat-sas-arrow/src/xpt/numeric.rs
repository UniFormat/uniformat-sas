//! Reject temporal precision loss before passing cells to the transport encoder.
use crate::{Error, Result};
use arrow_array::{cast::AsArray, types::*, ArrayRef};
use arrow_schema::{DataType, TimeUnit};
use uniformat_sas::temporal::{sas_datetime_to_unix_micros, sas_time_to_micros};

pub(crate) fn check(col: &ArrayRef, i: usize, value: f64) -> Result<()> {
    if let DataType::Date64 = col.data_type() {
        if col.as_primitive::<Date64Type>().value(i) % 86_400_000 != 0 {
            return Err(Error::Unsupported("XPT Date64 requires whole days"));
        }
    }
    let (raw, unit, datetime) = match col.data_type() {
        DataType::Timestamp(u, _) => {
            let raw = match u {
                TimeUnit::Second => col.as_primitive::<TimestampSecondType>().value(i),
                TimeUnit::Millisecond => col.as_primitive::<TimestampMillisecondType>().value(i),
                TimeUnit::Microsecond => col.as_primitive::<TimestampMicrosecondType>().value(i),
                TimeUnit::Nanosecond => col.as_primitive::<TimestampNanosecondType>().value(i),
            };
            (raw, u, true)
        }
        DataType::Time32(u) => {
            let raw = match u {
                TimeUnit::Second => col.as_primitive::<Time32SecondType>().value(i),
                _ => col.as_primitive::<Time32MillisecondType>().value(i),
            };
            (i64::from(raw), u, false)
        }
        DataType::Time64(u) => {
            let raw = match u {
                TimeUnit::Nanosecond => col.as_primitive::<Time64NanosecondType>().value(i),
                _ => col.as_primitive::<Time64MicrosecondType>().value(i),
            };
            (raw, u, false)
        }
        DataType::Duration(u) => {
            let raw = match u {
                TimeUnit::Second => col.as_primitive::<DurationSecondType>().value(i),
                TimeUnit::Millisecond => col.as_primitive::<DurationMillisecondType>().value(i),
                TimeUnit::Microsecond => col.as_primitive::<DurationMicrosecondType>().value(i),
                TimeUnit::Nanosecond => col.as_primitive::<DurationNanosecondType>().value(i),
            };
            (raw, u, false)
        }
        _ => return Ok(()),
    };
    let expected = match unit {
        TimeUnit::Second => raw.checked_mul(1_000_000),
        TimeUnit::Millisecond => raw.checked_mul(1000),
        TimeUnit::Microsecond => Some(raw),
        TimeUnit::Nanosecond => (raw % 1000 == 0).then_some(raw / 1000),
    };
    let decoded = if datetime {
        sas_datetime_to_unix_micros(value)
    } else {
        sas_time_to_micros(value)
    }
    .ok();
    if expected.is_none() || decoded != expected {
        return Err(Error::Unsupported(
            "XPT temporal value loses precision at microsecond resolution",
        ));
    }
    Ok(())
}
