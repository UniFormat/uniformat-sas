test_that("XPORT preserves tagged missing values, dates and attributes", {
  skip_if_not_installed("haven")
  path <- tempfile(fileext=".xpt")
  on.exit(unlink(path))
  x <- data.frame(N=c(1.5,NA_real_,haven::tagged_na("_"),haven::tagged_na(letters)),
                  TEXT=rep("ete",29), DAY=rep(as.Date("1960-01-01"),29))
  attr(x$N,"label") <- "Measure"
  write_xpt(x,path)
  y <- read_xpt(path)
  expect_equal(y$N[1],1.5,ignore_attr=TRUE)
  expect_equal(haven::na_tag(y$N),haven::na_tag(x$N))
  expect_equal(y$DAY,x$DAY,ignore_attr=TRUE)
  expect_equal(attr(y$N,"label"),"Measure")
  expect_equal(nrow(read_xpt(path,n_max=2,col_select="N")),2)
  expect_error(read_xpt(path,max_bytes=1),class="sas7bdat_resource_error")
})

test_that("XPORT v5 interoperates with independent R haven writer and reader", {
  skip_if_not_installed("haven")
  ours <- tempfile(fileext=".xpt"); reference <- tempfile(fileext=".xpt")
  on.exit(unlink(c(ours,reference)))
  x <- data.frame(VALUE=c(1,-1,0.5,100,NA_real_),TEXT=c("one","minus","half","cent","missing"))
  attr(x$VALUE,"label") <- "Number"
  write_xpt(x,ours)
  independent <- haven::read_xpt(ours)
  expect_equal(independent$VALUE,x$VALUE)
  expect_equal(independent$TEXT,x$TEXT)
  haven::write_xpt(x,reference,version=5,name="DATASET")
  decoded <- read_xpt(reference)
  expect_equal(decoded$VALUE,x$VALUE)
  expect_equal(decoded$TEXT,x$TEXT)
  expect_equal(names(decoded),names(x))
})

test_that("XPORT encodings and atomic errors are explicit", {
  path <- tempfile(fileext=".xpt"); on.exit(unlink(path))
  write_xpt(data.frame(N=1,TEXT="été"),path,encoding="latin1")
  expect_equal(read_xpt(path,encoding="latin1")$TEXT,"été",ignore_attr=TRUE)
  expect_error(read_xpt(path),"UTF-8")
  before <- readBin(path,"raw",n=file.info(path)$size)
  expect_error(write_xpt(data.frame(N=c(1,Inf)),path))
  expect_identical(readBin(path,"raw",n=file.info(path)$size),before)
})
