use super::{
    write_xpt_batches, xpt_schema, MemberSelection, XptBatchReader, XptReadOptions, XptWriteOptions,
};
use arrow_schema::SchemaRef;
use std::{collections::BTreeMap, path::Path, sync::Arc};
use uniformat_core::{
    BatchStream, Error, Format, ReadRequest, Result, TableInfo, WriteRequest, WriteSummary,
};
use uniformat_sas::xpt::{TextEncoding, Version};
use uniformat_sas_io::{FileSource, XptFile};

fn error(e: crate::Error) -> Error {
    Error::format("xpt", e)
}
fn encoding(v: &str) -> Result<TextEncoding> {
    match v.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(TextEncoding::Utf8),
        "latin1" | "latin-1" | "iso-8859-1" => Ok(TextEncoding::Latin1),
        "wlatin1" | "cp1252" | "windows-1252" => Ok(TextEncoding::Windows1252),
        _ => Err(Error::option(
            "encoding",
            "expected utf-8, latin1 or windows-1252",
        )),
    }
}
/// Parse common Arrow options plus an explicit transport encoding and member selector.
pub fn read_options(options: &BTreeMap<String, String>) -> Result<XptReadOptions> {
    let mut common = options.clone();
    let mut opts = XptReadOptions::default();
    if let Some(v) = common.remove("encoding") {
        opts.open.encoding = encoding(&v)?;
    }
    let name = common.remove("member");
    let index = common.remove("member_index");
    if name.is_some() && index.is_some() {
        return Err(Error::option(
            "member",
            "member and member_index are mutually exclusive",
        ));
    }
    if let Some(v) = name {
        opts.member = MemberSelection::Name(v);
    }
    if let Some(v) = index {
        opts.member = MemberSelection::Index(
            v.parse()
                .map_err(|_| Error::option("member_index", "expected a zero-based index"))?,
        );
    }
    for (k, v) in [("invalid_bytes", "error"), ("out_of_range", "error")] {
        common.entry(k.into()).or_insert(v.into());
    }
    if common.get("invalid_bytes").is_some_and(|s| s == "replace") {
        return Err(Error::option(
            "invalid_bytes",
            "XPT supports error or binary",
        ));
    }
    opts.read = crate::read_options(&common)?;
    Ok(opts)
}
/// Parse deterministic XPORT write options (v5 default; v8 explicit). Unsupported options are rejected.
pub fn write_options(options: &BTreeMap<String, String>) -> Result<XptWriteOptions> {
    let mut opts = XptWriteOptions::default();
    for (k, v) in options {
        match k.as_str() {
            "encoding" => opts.write.encoding = encoding(v)?,
            "table_name" => opts.write.table_name.clone_from(v),
            "table_label" => opts.write.table_label.clone_from(v),
            "created" => opts.write.created.clone_from(v),
            "modified" => opts.write.modified.clone_from(v),
            "missing_suffix" if !v.is_empty() => opts.missing_suffix.clone_from(v),
            "max_buffer_bytes" => {
                opts.max_buffer_bytes = v
                    .parse()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or_else(|| Error::option(k, "expected a positive byte budget"))?
            }
            "version" if v == "5" => opts.write.version = Version::V5,
            "version" if v == "8" => opts.write.version = Version::V8,
            "row_count" => {
                opts.write.row_count =
                    Some(v.parse().map_err(|_| {
                        Error::option(k, "expected a nonnegative observation count")
                    })?)
            }
            _ => {
                return Err(Error::option(
                    k,
                    "unknown or unsupported option for XPORT (version must be 5 or 8)",
                ))
            }
        }
    }
    Ok(opts)
}
/// SAS Transport format, registered together with sas7bdat.
#[derive(Debug, Clone, Copy, Default)]
pub struct XptFormat;
impl Format for XptFormat {
    fn name(&self) -> &'static str {
        "xpt"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["xpt"]
    }
    fn can_write(&self) -> bool {
        true
    }
    fn info(&self, path: &Path, request: &ReadRequest) -> Result<TableInfo> {
        let opts = read_options(&request.options)?;
        let file = XptFile::open(FileSource::open(path)?, opts.open.clone())
            .map_err(|e| error(e.into()))?;
        // The first member is available from headers alone. Selecting later members
        // explicitly requires a sequential index; do not invent a row count.
        let selected = match &opts.member {
            MemberSelection::First | MemberSelection::Index(0) => None,
            _ => Some(super::reader::select(&file, &opts.member).map_err(error)?),
        };
        let meta = selected.as_ref().map_or(file.metadata(), |m| m.metadata());
        let schema = xpt_schema(meta, &opts).map_err(error)?;
        let schema = match crate::format::projection(&schema, request.columns.as_ref())? {
            Some(p) => Arc::new(schema.project(&p)?),
            None => schema,
        };
        Ok(TableInfo {
            schema,
            row_count: None,
        })
    }
    fn read(&self, path: &Path, request: &ReadRequest) -> Result<BatchStream> {
        let opts = read_options(&request.options)?;
        let file = XptFile::open(FileSource::open(path)?, opts.open.clone())
            .map_err(|e| error(e.into()))?;
        let member = super::reader::select(&file, &opts.member).map_err(error)?;
        let schema = xpt_schema(member.metadata(), &opts).map_err(error)?;
        let projection = crate::format::projection(&schema, request.columns.as_ref())?;
        Ok(Box::new(
            XptBatchReader::with_member(file, member, opts, projection, request.limit)
                .map_err(error)?,
        ))
    }
    fn write(
        &self,
        path: &Path,
        schema: SchemaRef,
        batches: BatchStream,
        request: &WriteRequest,
    ) -> Result<WriteSummary> {
        let rows = write_xpt_batches(path, schema, batches, write_options(&request.options)?)
            .map_err(error)?;
        Ok(WriteSummary {
            rows,
            warnings: Vec::new(),
        })
    }
}
