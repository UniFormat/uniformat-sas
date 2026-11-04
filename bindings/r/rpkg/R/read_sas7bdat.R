#' Erreur du driver sas7bdat
#'
#' Condition de classe `sas7bdat_error` (et `sas7bdat_io_error`, `sas7bdat_format_error` ou
#' `sas7bdat_usage_error`), ADR 0007.
#' @noRd
sas7bdat_stop <- function(class, message, call = sys.call(-1)) {
  stop(structure(
    class = c(class, "sas7bdat_error", "error", "condition"),
    list(message = message, call = call)
  ))
}

sas7bdat_read_raw <- function(path, col_select, n_max, encoding, trim_ws, strict, max_bytes = 256 * 1024^2) {
  if (!is.character(path) || length(path) != 1L || is.na(path)) {
    sas7bdat_stop("sas7bdat_usage_error", "`path` doit etre une chaine de caracteres")
  }
  path <- path.expand(path)
  if (!is.null(col_select) && !is.character(col_select)) {
    sas7bdat_stop("sas7bdat_usage_error", "`col_select` doit etre un vecteur de noms de colonnes")
  }
  # une panique du coeur est rattrapee par extendr (condition R) : on la rend en `sas7bdat_internal_error`
  res <- tryCatch(
    sas7bdat_read_native(path, col_select, as.double(n_max), encoding, isTRUE(trim_ws), isTRUE(strict), as.double(max_bytes)),
    error = function(e) sas7bdat_stop("sas7bdat_internal_error", conditionMessage(e))
  )
  if (!isTRUE(res$ok)) {
    sas7bdat_stop(res$class, res$message)
  }
  for (w in res$warnings) {
    warning(w, call. = FALSE)
  }
  res
}

#' Lit un fichier SAS sas7bdat
#'
#' @param path Chemin du fichier `.sas7bdat`.
#' @param col_select Noms des colonnes a garder (vecteur de caracteres).
#' @param n_max Nombre maximal de lignes (`Inf` : toutes).
#' @param encoding Encodage impose (par exemple `"UTF-8"`), corrige un octet 70 non fiable.
#' @param trim_ws Retire les blancs de fin des chaines.
#' @param max_bytes Budget conservateur des colonnes de sortie en octets (256 Mio par defaut).
#' @param strict Les cas non verifies par la specification sont des erreurs.
#' @return Un `data.frame`. Attributs de colonne : `label`, `format.sas`, `informat.sas`. Les manquants speciaux
#'   (`._`, `.A` a `.Z`) sont des NA etiquetes lisibles par `haven::na_tag()`. Attributs de table : `label`,
#'   `sas7bdat.created`, `sas7bdat.encoding`, `sas7bdat.compression`.
#' @export
read_sas7bdat <- function(path, col_select = NULL, n_max = Inf, encoding = NULL, trim_ws = TRUE, strict = FALSE, max_bytes = 256 * 1024^2) {
  res <- sas7bdat_read_raw(path, col_select, n_max, encoding, trim_ws, strict, max_bytes)
  cols <- res$columns
  names(cols) <- res$names
  df <- structure(
    cols,
    row.names = c(NA_integer_, -res$n_rows),
    class = "data.frame"
  )
  for (i in seq_along(res$attr_names)) {
    attr(df, res$attr_names[[i]]) <- res$attr_values[[i]]
  }
  df
}

#' Metadonnees d'une table SAS (aucune ligne n'est lue)
#'
#' @inheritParams read_sas7bdat
#' @return Une liste : `table` (attributs de la table) et `columns` (`data.frame` : nom, type, label,
#'   `format.sas`, `informat.sas`).
#' @export
sas7bdat_meta <- function(path, encoding = NULL, strict = FALSE) {
  res <- sas7bdat_read_raw(path, NULL, 0, encoding, TRUE, strict)
  cols <- res$columns
  names(cols) <- res$names
  attr_or_na <- function(x, a) {
    v <- attr(x, a, exact = TRUE)
    if (is.null(v)) NA_character_ else v
  }
  list(
    table = stats::setNames(as.list(res$attr_values), res$attr_names),
    columns = data.frame(
      name = res$names,
      type = vapply(cols, function(x) if (is.character(x)) "char" else "num", ""),
      label = vapply(cols, attr_or_na, "", a = "label"),
      format.sas = vapply(cols, attr_or_na, "", a = "format.sas"),
      informat.sas = vapply(cols, attr_or_na, "", a = "informat.sas"),
      stringsAsFactors = FALSE,
      row.names = NULL
    )
  )
}

#' Ecrit un fichier SAS sas7bdat
#'
#' Ecriture transactionnelle : aucun fichier n'est laisse en cas d'erreur.
#'
#' @param data Un `data.frame`. Types : numerique, entier, logique, caractere, facteur, `Date`, `POSIXct`,
#'   `difftime`/`hms`. Les attributs `label`, `format.sas` et `informat.sas` sont ecrits. Les NA etiquetes de `haven`
#'   (`haven::tagged_na()`) deviennent les manquants speciaux `._` et `.A` a `.Z`.
#' @param path Chemin de sortie.
#' @param compression `"none"` (defaut), `"char"` ou `"binary"`.
#' @param encoding Encodage du fichier : `"UTF-8"`, `"latin1"` ou `"wlatin1"`.
#' @param page_length Taille de page demandee (agrandie si les metadonnees ou une ligne n'y tiennent pas).
#' @param on_unrepresentable `"error"` (defaut), `"replace"` ou `"truncate"` : valeur non representable.
#' @return `path`, de facon invisible.
#' @export
write_sas7bdat <- function(data, path, compression = c("none", "char", "binary"), encoding = "UTF-8",
                           page_length = 65536L, on_unrepresentable = c("error", "replace", "truncate")) {
  compression <- match.arg(compression)
  on_unrepresentable <- match.arg(on_unrepresentable)
  if (!is.data.frame(data) || ncol(data) == 0L) {
    sas7bdat_stop("sas7bdat_usage_error", "`data` doit etre un data.frame d'au moins une colonne")
  }
  if (!is.character(path) || length(path) != 1L || is.na(path)) {
    sas7bdat_stop("sas7bdat_usage_error", "`path` doit etre une chaine de caracteres")
  }
  n <- ncol(data)
  cols <- vector("list", n)
  classes <- character(n)
  attr_or_na <- function(x, a) {
    v <- attr(x, a, exact = TRUE)
    if (is.null(v) || length(v) != 1L || is.na(v)) NA_character_ else as.character(v)
  }
  for (i in seq_len(n)) {
    x <- data[[i]]
    if (inherits(x, "POSIXct")) {
      cols[[i]] <- as.double(x); classes[[i]] <- "datetime"
    } else if (inherits(x, "Date")) {
      cols[[i]] <- as.double(unclass(x)); classes[[i]] <- "date"
    } else if (inherits(x, "difftime")) {
      cols[[i]] <- as.double(x, units = "secs"); classes[[i]] <- "time"
    } else if (is.factor(x)) {
      cols[[i]] <- as.character(x); classes[[i]] <- "char"
    } else if (is.character(x)) {
      cols[[i]] <- as.character(x); classes[[i]] <- "char"
    } else if (is.numeric(x) || is.logical(x)) {
      cols[[i]] <- as.double(x); classes[[i]] <- "num"
    } else {
      sas7bdat_stop("sas7bdat_usage_error", sprintf("colonne %s : type %s non pris en charge", names(data)[[i]], class(x)[[1]]))
    }
  }
  labels <- vapply(data, attr_or_na, "", a = "label", USE.NAMES = FALSE)
  formats <- vapply(data, attr_or_na, "", a = "format.sas", USE.NAMES = FALSE)
  informats <- vapply(data, attr_or_na, "", a = "informat.sas", USE.NAMES = FALSE)
  res <- tryCatch(
    sas7bdat_write_native(path.expand(path), names(data), cols, classes, labels, formats, informats, compression,
                          encoding, as.integer(page_length), on_unrepresentable),
    error = function(e) sas7bdat_stop("sas7bdat_internal_error", conditionMessage(e))
  )
  if (!isTRUE(res$ok)) {
    sas7bdat_stop(res$class, res$message)
  }
  for (w in res$warnings) {
    warning(w, call. = FALSE)
  }
  invisible(path)
}
