# Critere 3 de J5 : sur les fichiers du corpus lisibles par haven, valeurs egales (all.equal), attributs `label` et
# `format.sas` egaux, NA etiquetes relus par haven::na_tag(). Saute sans UNIFORMAT_CORPUS ou sans haven.

corpus_root <- function() {
  root <- Sys.getenv("UNIFORMAT_CORPUS", Sys.getenv("SAS7BDAT_CORPUS", ""))
  if (!nzchar(root) || !dir.exists(root)) testthat::skip("UNIFORMAT_CORPUS absent")
  root
}

corpus_files <- function(root) {
  unlist(lapply(c("files", "external"), function(top) {
    d <- file.path(root, top)
    if (dir.exists(d)) list.files(d, pattern = "\\.sas7bdat$", recursive = TRUE, full.names = TRUE) else character()
  }))
}

# U22 (10 par. 5.4) : datareader/test16 a test21 portent un code d'encodage faux mais contiennent de l'UTF-8
encoding_for <- function(path) {
  stem <- sub("\\.sas7bdat$", "", basename(path))
  n <- suppressWarnings(as.integer(sub("^test", "", stem)))
  if (grepl("datareader", path) && grepl("^test[0-9]+$", stem) && !is.na(n) && n >= 16 && n <= 21) "UTF-8" else NULL
}

tags <- function(x) {
  vapply(seq_along(x), function(i) {
    t <- haven::na_tag(x[i])
    if (is.na(t)) "" else t
  }, "")
}

# Ecarts entre notre lecture et celle de haven (vecteur vide = identiques)
diff_with_haven <- function(path) {
  enc <- encoding_for(path)
  ours <- suppressWarnings(uniformatsas::read_sas7bdat(path, encoding = enc))
  theirs <- suppressWarnings(haven::read_sas(path, encoding = enc))
  out <- character()
  # ecart documente : haven (ReadStat) ne masque pas les lignes supprimees ; nous si (bitmap des suppressions)
  deleted <- as.integer(attr(ours, "sas7bdat.deleted_rows", exact = TRUE))
  if (nrow(ours) != nrow(theirs)) {
    if (deleted > 0 && nrow(ours) + deleted == nrow(theirs)) return(character())
    out <- c(out, sprintf("%d lignes contre %d", nrow(ours), nrow(theirs)))
  }
  if (!identical(names(ours), names(theirs))) out <- c(out, "noms de colonnes differents")
  if (length(out)) return(out)
  for (nm in names(ours)) {
    a <- ours[[nm]]; b <- theirs[[nm]]
    la <- attr(a, "label", exact = TRUE); lb <- attr(b, "label", exact = TRUE)
    if (!identical(la, lb)) out <- c(out, sprintf("%s : label %s contre %s", nm, deparse(la), deparse(lb)))
    fa <- attr(a, "format.sas", exact = TRUE); fb <- attr(b, "format.sas", exact = TRUE)
    # ecart documente : pour les fichiers de PROC IMPORT (datareader, pandas) haven rend le nom sans largeur
    # ("BEST", "$", "MMDDYY") ; nous gardons la largeur du fichier ("BEST12", "$9", "MMDDYY10")
    same_name <- !is.null(fa) && !is.null(fb) && identical(sub("[0-9.]+$", "", fa), fb)
    if (!identical(fa, fb) && !same_name) out <- c(out, sprintf("%s : format.sas %s contre %s", nm, deparse(fa), deparse(fb)))
    if (!identical(class(a), class(b))) {
      out <- c(out, sprintf("%s : classe %s contre %s", nm, paste(class(a), collapse = "/"), paste(class(b), collapse = "/")))
      next
    }
    if (is.numeric(a)) {
      ea <- unclass(a); eb <- unclass(b)
      attributes(ea) <- NULL; attributes(eb) <- NULL
      r <- all.equal(ea, eb, tolerance = 1e-12)
      if (!isTRUE(r)) out <- c(out, sprintf("%s : %s", nm, paste(r, collapse = " ; ")))
      if (!identical(tags(ea), tags(eb))) out <- c(out, sprintf("%s : etiquettes de NA differentes", nm))
    } else {
      attributes(a) <- NULL; attributes(b) <- NULL
      # ecart documente : octets invalides pour l'encodage (compression_runs_*) : remplaces par U+FFFD ici, gardes
      # bruts par haven
      if (!identical(a, b) && !grepl("compression_runs", path)) out <- c(out, sprintf("%s : texte different", nm))
    }
  }
  out
}

test_that("les valeurs, labels et formats egalent ceux de haven sur le corpus", {
  skip_if_not_installed("haven")
  root <- corpus_root()
  files <- corpus_files(root)
  expect_gte(length(files), 232)
  compared <- 0L; unreadable <- character(); failures <- list()
  for (f in files) {
    enc <- encoding_for(f)
    ok <- tryCatch({ suppressWarnings(haven::read_sas(f, encoding = enc)); TRUE }, error = function(e) FALSE)
    if (!ok) { unreadable <- c(unreadable, basename(f)); next }
    d <- tryCatch(diff_with_haven(f), error = function(e) paste("erreur :", conditionMessage(e)))
    compared <- compared + 1L
    if (length(d)) failures[[sub(paste0(root, "/"), "", f, fixed = TRUE)]] <- utils::head(d, 3)
  }
  message(sprintf("%d fichiers compares, %d illisibles par haven", compared, length(unreadable)))
  if (length(unreadable)) message("illisibles par haven : ", paste(unreadable, collapse = ", "))
  expect_gte(compared, 150)
  if (length(failures)) {
    message(paste(vapply(names(failures), function(n) paste0(n, " :\n    ", paste(failures[[n]], collapse = "\n    ")), ""), collapse = "\n"))
  }
  expect_length(failures, 0)
})

test_that("les 28 manquants speciaux se relisent avec haven::na_tag()", {
  skip_if_not_installed("haven")
  root <- corpus_root()
  f <- file.path(root, "files", "valeurs", "valeurs_manquants_no.sas7bdat")
  x <- uniformatsas::read_sas7bdat(f)$x
  expect_length(x, 28)
  expect_true(all(is.na(x)))
  expect_equal(tags(x), c("", "_", letters))
  expect_true(haven::is_tagged_na(x[3], "a"))
  expect_false(haven::is_tagged_na(x[1]))
  # un NA etiquete construit par haven est reconnu a l'identique
  expect_identical(tags(haven::tagged_na("b")), "b")
})

test_that("les attributs de table et de colonne sont ceux de la specification", {
  root <- corpus_root()
  f <- file.path(root, "files", "metadonnees", list.files(file.path(root, "files", "metadonnees"), pattern = "\\.sas7bdat$")[1])
  df <- uniformatsas::read_sas7bdat(f)
  expect_true(all(c("sas7bdat.encoding", "sas7bdat.compression", "sas7bdat.created") %in% names(attributes(df))))
  meta <- uniformatsas::sas7bdat_meta(f)
  expect_equal(meta$columns$name, names(df))
  expect_equal(nrow(uniformatsas::read_sas7bdat(f, n_max = 0)), 0L)
  expect_error(uniformatsas::read_sas7bdat(f, col_select = "nope"), class = "sas7bdat_usage_error")
})

test_that("projection et n_max", {
  root <- corpus_root()
  f <- file.path(root, "files", "taille", "taille_n50000_no.sas7bdat")
  all <- uniformatsas::read_sas7bdat(f)
  expect_equal(nrow(all), 50000L)
  nm <- names(all)
  part <- uniformatsas::read_sas7bdat(f, col_select = rev(nm)[1:2], n_max = 7)
  expect_equal(names(part), rev(nm)[1:2])
  expect_equal(nrow(part), 7L)
})

test_that("les erreurs sont des conditions sas7bdat_error", {
  expect_error(uniformatsas::read_sas7bdat(tempfile(fileext = ".sas7bdat")), class = "sas7bdat_io_error")
  junk <- tempfile(fileext = ".sas7bdat")
  writeBin(as.raw(rep(120L, 1000)), junk)
  expect_error(uniformatsas::read_sas7bdat(junk), class = "sas7bdat_format_error")
  expect_error(uniformatsas::read_sas7bdat(junk), class = "sas7bdat_error")
})

test_that("des fichiers corrompus ne font jamais tomber R", {
  root <- corpus_root()
  set.seed(20261104)
  sources <- Filter(function(f) file.size(f) < 400000, corpus_files(root))
  sources <- sample(sources, 40)
  tmp <- tempfile(fileext = ".sas7bdat")
  runs <- 0L; other <- character()
  for (src in sources) {
    data <- readBin(src, "raw", file.size(src))
    for (k in 1:12) {
      buf <- data
      if (k <= 3) buf <- buf[seq_len(sample(0:(length(buf) - 1), 1))]
      else if (k <= 8) { i <- sample(length(buf), sample(1:20, 1)); buf[i] <- as.raw(sample(0:255, length(i), TRUE)) }
      else { i <- sample(min(length(buf), 20000), sample(1:40, 1)); buf[i] <- as.raw(sample(0:255, length(i), TRUE)) }
      writeBin(buf, tmp)
      runs <- runs + 1L
      res <- tryCatch({ suppressWarnings(uniformatsas::read_sas7bdat(tmp)); "ok" },
                      sas7bdat_internal_error = function(e) paste("erreur interne :", conditionMessage(e)),
                      sas7bdat_error = function(e) "sas7bdat_error",
                      error = function(e) paste("autre erreur :", conditionMessage(e)))
      if (!res %in% c("ok", "sas7bdat_error")) other <- c(other, paste(basename(src), k, res))
    }
  }
  expect_gte(runs, 400)
  expect_length(other, 0)
})

# ---- J6 : ecriture -------------------------------------------------------------------------------------------------

test_that("write_sas7bdat : aller-retour avec notre lecture et avec haven (types, attributs, NA etiquetes)", {
  skip_if_not_installed("haven")
  f <- tempfile(fileext = ".sas7bdat")
  on.exit(unlink(f))
  x <- c(1.5, NA, 3, haven::tagged_na("b"), haven::tagged_na("z"))
  df <- data.frame(
    id = 1:5,
    x = x,
    flag = c(TRUE, FALSE, NA, TRUE, TRUE),
    s = c("a", "bé", NA, "", "texte long"),
    d = as.Date("2020-01-01") + 0:4,
    t = as.POSIXct("2020-01-01 12:00:00", tz = "UTC") + 3600 * 0:4,
    h = hms::as_hms(3600 * 1:5),
    stringsAsFactors = FALSE
  )
  attr(df$x, "label") <- "Montant"
  attr(df$x, "format.sas") <- "COMMA10.2"
  for (compression in c("none", "char", "binary")) {
    uniformatsas::write_sas7bdat(df, f, compression = compression)
    ours <- uniformatsas::read_sas7bdat(f)
    theirs <- haven::read_sas(f)
    expect_equal(attr(ours, "sas7bdat.compression"), compression)
    expect_equal(ours$id, as.double(1:5)); expect_equal(theirs$id, as.double(1:5))
    expect_identical(tags(ours$x), c("", "", "", "b", "z"))
    expect_identical(tags(theirs$x), c("", "", "", "b", "z"))
    expect_equal(ours$x[c(1, 3)], c(1.5, 3)); expect_equal(theirs$x[c(1, 3)], c(1.5, 3))
    expect_true(is.na(ours$x[2]))
    expect_equal(ours$flag, c(1, 0, NA, 1, 1))
    expect_equal(ours$s, c("a", "bé", "", "", "texte long")); expect_equal(theirs$s, c("a", "bé", "", "", "texte long"))
    expect_s3_class(ours$d, "Date"); expect_equal(as.double(unclass(ours$d)), as.double(unclass(df$d))); expect_equal(as.double(unclass(theirs$d)), as.double(unclass(df$d)))
    expect_s3_class(ours$t, "POSIXct"); expect_equal(as.double(ours$t), as.double(df$t))
    expect_equal(as.double(theirs$t), as.double(df$t))
    expect_s3_class(ours$h, "hms"); expect_equal(as.double(ours$h), as.double(df$h))
    expect_equal(attr(ours$x, "label"), "Montant"); expect_equal(attr(theirs$x, "label"), "Montant")
    expect_equal(attr(ours$x, "format.sas"), "COMMA10.2"); expect_equal(attr(theirs$x, "format.sas"), "COMMA10.2")
    expect_equal(attr(ours$d, "format.sas"), "DATE9"); expect_equal(attr(theirs$d, "format.sas"), "DATE9")
  }
})

test_that("write_sas7bdat : encodages, erreurs typees et ecriture transactionnelle", {
  f <- tempfile(fileext = ".sas7bdat")
  on.exit(unlink(f))
  df <- data.frame(s = c("café", "été"), stringsAsFactors = FALSE)
  for (enc in c("UTF-8", "latin1", "wlatin1")) {
    uniformatsas::write_sas7bdat(df, f, encoding = enc)
    expect_equal(uniformatsas::read_sas7bdat(f)$s, df$s)
  }
  # euro absent de latin1 : erreur typee, aucun fichier
  g <- tempfile(fileext = ".sas7bdat")
  bad <- data.frame(s = "5€", stringsAsFactors = FALSE)
  expect_error(uniformatsas::write_sas7bdat(bad, g, encoding = "latin1"), class = "sas7bdat_format_error")
  expect_false(file.exists(g))
  expect_equal(length(list.files(dirname(g), pattern = paste0("^\\.", basename(g)))), 0L)
  suppressWarnings(uniformatsas::write_sas7bdat(bad, g, encoding = "latin1", on_unrepresentable = "replace"))
  expect_equal(uniformatsas::read_sas7bdat(g)$s, "5?")
  unlink(g)
  long <- data.frame(n = 1, stringsAsFactors = FALSE)
  names(long) <- strrep("n", 33)
  expect_error(uniformatsas::write_sas7bdat(long, g), class = "sas7bdat_error")
  expect_false(file.exists(g))
  expect_error(uniformatsas::write_sas7bdat(list(a = 1), g), class = "sas7bdat_usage_error")
  expect_error(uniformatsas::write_sas7bdat(data.frame(z = I(list(1))), g), class = "sas7bdat_usage_error")
})

test_that("write_sas7bdat : relecture du corpus par haven (lignes et valeurs inchangees)", {
  skip_if_not_installed("haven")
  root <- corpus_root()
  f <- file.path(root, "files", "base", "base_n100_mix_no.sas7bdat")
  src <- uniformatsas::read_sas7bdat(f)
  g <- tempfile(fileext = ".sas7bdat")
  on.exit(unlink(g))
  for (compression in c("none", "char", "binary")) {
    uniformatsas::write_sas7bdat(src, g, compression = compression)
    again <- uniformatsas::read_sas7bdat(g)
    hv <- haven::read_sas(g)
    expect_equal(nrow(again), nrow(src))
    for (nm in names(src)) {
      expect_equal(as.vector(unclass(again[[nm]])), as.vector(unclass(src[[nm]])))
      expect_equal(as.vector(unclass(hv[[nm]])), as.vector(unclass(src[[nm]])))
    }
  }
})
