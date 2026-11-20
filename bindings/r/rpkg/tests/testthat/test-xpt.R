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

test_that("XPORT v8 long metadata interoperates with haven in both directions", {
  skip_if_not_installed("haven")
  ours <- tempfile(fileext=".xpt"); reference <- tempfile(fileext=".xpt")
  on.exit(unlink(c(ours,reference)))
  x <- data.frame(same_prefix_first=c(1.25,-2.5,NA_real_),
                  same_prefix_second=c(strrep("x",250),"ete","tail"))
  attr(x$same_prefix_first,"label") <- strrep("a",100)
  attr(x$same_prefix_second,"label") <- strrep("b",256)
  for (extended in c(FALSE,TRUE)) {
    attr(x$same_prefix_first,"format.sas") <- if (extended) "LONG_FORMAT12.2" else NULL
    # Use long formats on all reference entries to avoid old ReadStat's mixed
    # LABELV9 writer bug; our mixed layout is covered by the Python reader test.
    attr(x$same_prefix_second,"format.sas") <- if (extended) "$LONG_FORMAT250." else NULL
    write_xpt(x,ours,version=8,table_name="LONG_DATASET_NAME")
    independent <- haven::read_xpt(ours)
    expect_equal(as.numeric(independent[[1]]),as.numeric(x[[1]]))
    expect_equal(as.character(independent[[2]]),as.character(x[[2]]))
    expect_equal(names(independent),names(x))
    expect_equal(attr(independent[[1]],"label"),attr(x[[1]],"label"))
    haven::write_xpt(x,reference,version=8,name="LONG_DATASET_NAME")
    decoded <- read_xpt(reference)
    expect_equal(as.numeric(decoded[[1]]),as.numeric(x[[1]]))
    expect_equal(as.character(decoded[[2]]),as.character(x[[2]]))
    expect_equal(attr(decoded[[2]],"label"),attr(x[[2]],"label"))
    expect_equal(attr(decoded,"xpt.version"),"8")
    expect_equal(attr(decoded,"xpt.name"),"LONG_DATASET_NAME")
  }
})

test_that("XPORT v8 counted blank tails are preserved and v5 remains default", {
  path <- tempfile(fileext=".xpt"); on.exit(unlink(path))
  x <- data.frame(long_blank_name=c("x","",""))
  expect_error(write_xpt(x,path))
  write_xpt(x,path,version=8)
  expect_equal(as.character(read_xpt(path)[[1]]),x[[1]])
  expect_error(write_xpt(x,path,version=7))
})
