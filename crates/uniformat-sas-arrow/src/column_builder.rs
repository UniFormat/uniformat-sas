//! Arrow builders shared by both SAS containers.
use crate::schema::Repr;
use arrow_array::{
    builder::{
        BinaryBuilder, Date32Builder, DurationMicrosecondBuilder, Float64Builder, StringBuilder,
        Time64MicrosecondBuilder, TimestampMicrosecondBuilder, TimestampSecondBuilder,
        UInt8Builder,
    },
    ArrayRef,
};
use std::sync::Arc;

pub(crate) enum Builder {
    Float64(Float64Builder),
    Date32(Date32Builder),
    TimestampMicro(TimestampMicrosecondBuilder),
    TimestampSecond(TimestampSecondBuilder),
    Time64Micro(Time64MicrosecondBuilder),
    DurationMicro(DurationMicrosecondBuilder),
    Utf8(StringBuilder),
    Binary(BinaryBuilder),
    Sidecar(UInt8Builder),
}

impl Builder {
    pub(crate) fn new(repr: Repr, capacity: usize) -> Self {
        match repr {
            Repr::Float64 => Self::Float64(Float64Builder::with_capacity(capacity)),
            Repr::Date32 => Self::Date32(Date32Builder::with_capacity(capacity)),
            Repr::TimestampMicro => {
                Self::TimestampMicro(TimestampMicrosecondBuilder::with_capacity(capacity))
            }
            Repr::TimestampSecond => {
                Self::TimestampSecond(TimestampSecondBuilder::with_capacity(capacity))
            }
            Repr::Time64Micro => {
                Self::Time64Micro(Time64MicrosecondBuilder::with_capacity(capacity))
            }
            Repr::DurationMicro => {
                Self::DurationMicro(DurationMicrosecondBuilder::with_capacity(capacity))
            }
            Repr::Utf8 => Self::Utf8(StringBuilder::with_capacity(
                capacity,
                capacity.saturating_mul(8),
            )),
            Repr::Binary => Self::Binary(BinaryBuilder::with_capacity(
                capacity,
                capacity.saturating_mul(8),
            )),
            Repr::SidecarU8 => Self::Sidecar(UInt8Builder::with_capacity(capacity)),
        }
    }

    pub(crate) fn finish(&mut self) -> ArrayRef {
        match self {
            Self::Float64(b) => Arc::new(b.finish()),
            Self::Date32(b) => Arc::new(b.finish()),
            Self::TimestampMicro(b) => Arc::new(b.finish()),
            Self::TimestampSecond(b) => Arc::new(b.finish()),
            Self::Time64Micro(b) => Arc::new(b.finish()),
            Self::DurationMicro(b) => Arc::new(b.finish()),
            Self::Utf8(b) => Arc::new(b.finish()),
            Self::Binary(b) => Arc::new(b.finish()),
            Self::Sidecar(b) => Arc::new(b.finish()),
        }
    }
}

/// Une colonne de sortie : la colonne SAS, son rôle et son constructeur.
pub(crate) struct OutColumn {
    pub(crate) source: usize,
    pub(crate) builder: Builder,
}
