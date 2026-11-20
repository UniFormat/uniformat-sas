use super::XptReadOptions;
use crate::schema::{column_plans, FieldPlan};
use crate::{Error, Result};
use arrow_schema::{Schema, SchemaRef};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uniformat_sas::xpt::Metadata;
use uniformat_sas::InvalidBytes;

pub(super) fn plans(
    meta: &Metadata,
    opts: &XptReadOptions,
    projection: Option<&[usize]>,
) -> Result<Vec<FieldPlan>> {
    if opts.read.batch_rows == 0
        || opts.read.missing_suffix.is_empty()
        || opts.read.encoding_override.is_some()
    {
        return Err(Error::Unsupported("XPT requires batch_rows > 0, nonempty missing_suffix and encoding through open.encoding"));
    }
    if opts.read.invalid_bytes == InvalidBytes::Replace {
        return Err(Error::Unsupported(
            "XPT supports strict text decoding or binary output",
        ));
    }
    let plans = column_plans(meta.columns(), true, &opts.read);
    let mut names = HashSet::new();
    if plans.iter().any(|p| !names.insert(p.field.name())) {
        return Err(Error::Unsupported(
            "XPT column and missing-sidecar names collide",
        ));
    }
    let count = projection.map_or(plans.len(), <[usize]>::len);
    if count > opts.read.max_output_columns {
        return Err(Error::ResourceLimit("too many XPT output columns"));
    }
    match projection {
        None => Ok(plans),
        Some(indices) => indices
            .iter()
            .map(|&i| plans.get(i).cloned().ok_or(Error::Projection(i)))
            .collect(),
    }
}
pub(super) fn from_plans(meta: &Metadata, plans: &[FieldPlan]) -> SchemaRef {
    let mut md = HashMap::new();
    md.insert("sas.table_name".into(), meta.header().name.clone());
    md.insert("sas.table_label".into(), meta.header().label.clone());
    md.insert(
        "xpt.version".into(),
        meta.header().version.number().to_string(),
    );
    md.insert("xpt.created".into(), meta.header().created.clone());
    md.insert("xpt.modified".into(), meta.header().modified.clone());
    Arc::new(Schema::new_with_metadata(
        plans.iter().map(|p| p.field.clone()).collect::<Vec<_>>(),
        md,
    ))
}
/// Arrow schema from validated metadata; no observation reads or invented row count.
pub fn xpt_schema(meta: &Metadata, opts: &XptReadOptions) -> Result<SchemaRef> {
    Ok(from_plans(meta, &plans(meta, opts, None)?))
}
