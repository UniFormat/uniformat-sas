test_that("forged row counts produce a resource error through the native R binding", {
  path <- tempfile(fileext = ".sas7bdat")
  on.exit(unlink(path))
  uniformatsas::write_sas7bdat(data.frame(x = 1), path)
  bytes <- readBin(path, what = "raw", n = file.info(path)$size)
  expect_identical(bytes[33L], as.raw(0x33))
  at <- grepRaw(as.raw(rep(0xf7, 4)), bytes, fixed = TRUE, all = TRUE)[1L]
  expect_false(is.na(at))
  expect_identical(bytes[at + 48L], as.raw(1))
  bytes[(at + 48L):(at + 55L)] <- as.raw(rep(255, 8))
  writeBin(bytes, path)
  expect_error(uniformatsas::read_sas7bdat(path), class = "sas7bdat_resource_error")
  expect_equal(uniformatsas::read_sas7bdat(path, n_max = 1)$x, 1)
})

test_that("output budgets apply through the exported R API", {
  path <- tempfile(fileext = ".sas7bdat")
  on.exit(unlink(path))
  uniformatsas::write_sas7bdat(data.frame(x = as.double(1:100), text = rep("synthetic", 100)), path)
  expect_error(uniformatsas::read_sas7bdat(path, max_bytes = 1024), class = "sas7bdat_resource_error")
  expect_equal(nrow(uniformatsas::read_sas7bdat(path, n_max = 0, max_bytes = 1024)), 0L)
  expect_equal(nrow(uniformatsas::read_sas7bdat(path)), 100L)
  expect_error(uniformatsas::read_sas7bdat(path, max_bytes = 0), class = "sas7bdat_usage_error")
})
