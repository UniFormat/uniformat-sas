# Security limits and validation

The 2026-11-04 hardening addresses malicious file metadata, amplified projections, local file permissions and CI dependencies. These limits complement validation of the SAS format; they do not certify an entire host or deployment.

## Reader limits

The R converter no longer allocates from the declared row count. It checks a conservative 256 MiB output budget (`max_bytes`) before importing and while appending actual rows, and reserves vector capacity fallibly. An excessive declaration returns `sas7bdat_resource_error`. `n_max` and `col_select` can reduce a legitimate import; raise the budget only for trusted inputs. The budget covers conversion buffers, not the interpreter's total memory.

Arrow readers limit output to 16384 columns and a conservative 64 MiB of batch buffers by default, including UTF-8 expansion and buffer growth. Initial capacity is small, the target batch size adapts to the schema, and every appended row is checked. Decoding is page-granular: a page exceeding the budget is rejected rather than emitted partially. A terminal reader error never returns a partial batch on a later call. Rust callers can set `ReadOptions::max_batch_bytes` and `max_output_columns`; the format/CLI interface accepts the same keys through `--read-opt`.

Python `read_sas_table` also limits the sum of retained Arrow batch buffers to 256 MiB (`max_table_bytes`), raising `MemoryError` on excess. `read_sas` remains a streaming alternative; consumers are responsible for not retaining an unlimited number of batches. A complete-table conversion can temporarily hold one additional bounded batch before rejecting it.

The object-store stream decodes only one output batch per poll instead of accumulating every expanded batch of a fetched group. Input prefetch remains separate and bounded by group size. Fetch ranges are generated lazily; a full remote page plan is bounded using the metadata budget before allocating its vectors.

Remote object opening limits its metadata prefix to 64 MiB (`max_metadata_bytes`). Our adapters use object_store 0.14.2, which uses fixed quick-xml versions. DataFusion 55's internal object_store 0.13 remains independent and has no HTTP/cloud features in the audited graph. `SasTableProvider::try_new` takes our object_store 0.14 types, not DataFusion's internal registry types. HTTP and SQL integration must be tested when updating either dependency.

## Flight admission and deployment

Server-controlled `ServiceLimits` default to 4 concurrent requests, 64 KiB command/ticket/criteria payloads, 1024 output columns, 8 MiB per batch, 8 MiB of encoded metadata per response/listing, a 30-second request deadline, and listings of at most 1000 files / 10000 visited entries. Repeated projection names are rejected. Tickets cannot override these limits. Blocking tasks keep their admission permit until they actually exit, even after a timeout. Streamed responses retain a permit until completion/drop; producers stop on cancellation/deadline and queue at most one batch. Output budgets exclude input buffers, metadata and transport buffers; use process/container memory limits as well.

Flight additionally restricts source pages/rows to 1 MiB and text metadata to 4 MiB. Paths are canonicalized within the configured root and outgoing symlinks are rejected. The root and its parents must not be writable by untrusted users: pathname validation cannot eliminate races against a local directory writer.

The binary listens on loopback by default. Binding elsewhere requires explicit `--trusted-proxy`: deploy it only on a private network behind an authenticated TLS reverse proxy with connection/stream deadlines and request limits. This switch does not add TLS to Flight. A bearer token is optional when authentication is delegated; supply it through `UNIFORMAT_FLIGHT_TOKEN` rather than command-line arguments. Empty/invalid tokens are rejected and token bytes use a constant-time comparison. Do not expose an unauthenticated plaintext listener publicly.

## File writes

The common AtomicFile uses private random temporary files. New Unix files are 0600; replacements preserve ordinary permission bits observed at commit. Symlink destinations are rejected. The API does not copy ownership, extended ACLs or special mode bits; review the destination directory policy on the target platform.

## Tests and audits

Run the default suite without any external corpus:

```sh
cargo test --locked --workspace --exclude uniformat-sas-py
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny --locked --all-features check advisories --warn unmaintained
```

Mandatory generated fixtures cover all three compression modes, declared-row overflow in R, Arrow allocation limits, terminal errors, 204 deterministic parser mutations, real Flight RPC authentication/path/ticket/projection checks, deadlines, admission and cancellation. The core and CLI suites test private file replacement end to end. Python and R package tests also exercise generated tables.

Corpus-dependent Rust tests are explicitly ignored by default and fail if invoked without a corpus. Run the optional compatibility suite with `UNIFORMAT_CORPUS=/trusted/corpus cargo test --locked --workspace --exclude uniformat-sas-py -- --ignored`. Some ignored tests have separate prerequisites (for example benchmarks); select the intended test targets when necessary. Synthetic fixtures exercise security boundaries but do not replace an independent SAS compatibility oracle.

All workspace dependency locks are tracked; CI checks out sibling repositories at full commit SHAs before either builds or audits. Actions and reusable workflows are pinned, permissions are read-only, and checkout credentials are not retained. Python CI uses hashed dependency locks. `deny.toml` includes development dependencies and hides no advisory. CI keeps maintenance-only advisories visible as warnings (`--warn unmaintained`); vulnerabilities, unsoundness and yanked versions remain errors. The informational RUSTSEC-2024-0436 warning remains visible for paste, a build-time macro required by extendr 0.9: no maintained compatible upstream release is available at this audit date. It is a maintenance warning, not a detected runtime vulnerability; recheck it when updating extendr.

Re-run dependency and secret scans after changing dependencies or before publishing. Local validation does not configure GitHub branch protection, secret protection or production proxy settings.
