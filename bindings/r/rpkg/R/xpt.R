#' Read SAS Transport v5
#' @param path Path to the XPT file.
#' @param col_select Column names to retain.
#' @param n_max Maximum observations, Inf for all.
#' @param encoding UTF-8 (strict default), latin1 or windows-1252.
#' @param trim_ws Remove trailing spaces.
#' @param max_bytes Conservative output budget in bytes.
#' @param member Dataset name or one-based index (default 1).
#' @return A data.frame with SAS labels, formats and haven-compatible tagged NA.
#' @export
read_xpt <- function(path, col_select=NULL, n_max=Inf, encoding="UTF-8", trim_ws=TRUE,
                     max_bytes=256*1024^2, member=1) {
  stopifnot(is.character(path), length(path)==1L, !is.na(path),
            is.null(col_select) || is.character(col_select), length(member)==1L)
  if (is.numeric(member)) member <- as.double(member)
  res <- .Call(wrap__xpt_read_native, path.expand(path), col_select, as.double(n_max),
               encoding, isTRUE(trim_ws), as.double(max_bytes), member)
  if (!isTRUE(res$ok)) sas7bdat_stop(res$class, res$message)
  cols <- res$columns
  names(cols) <- res$names
  df <- structure(cols, row.names=c(NA_integer_, -res$n_rows), class="data.frame")
  for (i in seq_along(res$attr_names)) attr(df, res$attr_names[[i]]) <- res$attr_values[[i]]
  df
}

#' Write SAS Transport v5 atomically
#' @param data A data.frame with numeric, character, Date, POSIXct or hms columns.
#' @param path Output file.
#' @param encoding UTF-8, latin1 or windows-1252.
#' @param table_name Dataset name, at most 8 ASCII bytes.
#' @param table_label Dataset label, at most 40 encoded bytes.
#' @param created,modified Deterministic header dates, ddMMMyy:hh:mm:ss.
#' @return path, invisibly. Invalid widths or unrepresentable values cause an error.
#' @export
write_xpt <- function(data, path, encoding="UTF-8", table_name=NULL, table_label=NULL,
                      created=NULL, modified=NULL) {
  stopifnot(is.data.frame(data), ncol(data)>0L, is.character(path), length(path)==1L, !is.na(path))
  fallback <- function(explicit, attribute, default) {
    if (!is.null(explicit)) return(explicit)
    v <- attr(data, attribute, exact=TRUE)
    if (is.null(v)) default else v
  }
  p <- sas_prepare_write(data)
  res <- .Call(wrap__xpt_write_native, path.expand(path), names(data), p$columns, p$classes,
               p$labels, p$formats, p$informats, encoding,
               fallback(table_name,"xpt.name","DATASET"), fallback(table_label,"label",""),
               fallback(created,"xpt.created","01JAN00:00:00:00"),
               fallback(modified,"xpt.modified","01JAN00:00:00:00"))
  if (!isTRUE(res$ok)) sas7bdat_stop(res$class,res$message)
  invisible(path)
}
