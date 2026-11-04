# Enveloppes .Call des fonctions natives (equivalent de rextendr::document()).
#' @useDynLib uniformatsas, .registration = TRUE
#' @noRd
sas7bdat_read_native <- function(path, col_select, n_max, encoding, trim_ws, strict) {
  .Call(wrap__sas7bdat_read_native, path, col_select, n_max, encoding, trim_ws, strict)
}

sas7bdat_write_native <- function(path, names, columns, classes, labels, formats, informats, compression, encoding,
                                  page_length, on_unrepresentable) {
  .Call(wrap__sas7bdat_write_native, path, names, columns, classes, labels, formats, informats, compression, encoding,
        page_length, on_unrepresentable)
}
