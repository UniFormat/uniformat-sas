# XPORT v5 fixtures

`v5-synthetic.xpt` is generated solely by this repository (MIT OR Apache-2.0),
without SAS or any licensed corpus. `v5-blank-tail.xpt` exercises a final blank
character field with the same five rows. It contains five VALUE/TEXT rows, a numeric
missing value, UTF-8 text, labels and deterministic timestamps.

Regenerate from the repository root:

```sh
cargo run -p uniformat-sas-io --example xpt_fixture -- tests/data/xpt/v5-synthetic.xpt
cargo run -p uniformat-sas-io --example xpt_fixture -- tests/data/xpt/v5-blank-tail.xpt --blank-tail
```

Independent interoperability check (Python >= 3.10, pandas and pyreadstat installed):

```sh
python tests/data/xpt/interop.py /tmp/reference-v5.xpt
UNIFORMAT_XPT_REFERENCE=/tmp/reference-v5.xpt cargo test -p uniformat-sas-io --test xpt independent_reference_v5 -- --ignored
```

The script compares our file with pandas and ReadStat, then optionally generates a
reference with ReadStat for the reverse-direction Rust test. Reference data are
synthetic too, but that generated file is intentionally not committed. The first lot used only these synthetic references. The subsequent integration
was validated against an externally supplied SAS 9.4 corpus; see
[the validation report](../../../docs/xpt-sas-validation.md).

`validate_sas_reference.py /path/sas-xpt-v5-reference` checks that corpus through
the Python/Arrow binding, including 15 members, exact IEEE values, missing codes,
encodings, metadata, native dates and round trips. Set `UNIFORMAT_XPT_SAS_CORPUS`
to enable `bindings/python/tests/test_xpt_sas_reference.py`. Never commit the
external archive, extracted XPT files or SAS reference CSVs.

Seed the fuzz target with `v5-synthetic.xpt`; use a temporary corpus directory so
mutated artifacts never enter this fixture directory:

```sh
mkdir -p /tmp/uniformat-xpt-corpus
cp tests/data/xpt/v5-synthetic.xpt /tmp/uniformat-xpt-corpus/
cargo +nightly fuzz run fuzz_xpt /tmp/uniformat-xpt-corpus -- -max_len=1048576
```

The main interoperability fixture uses 16-byte observations with a nonempty last
text field. pandas 2.3.3 counts all blank 8-byte words in the last physical record
as padding, which can drop a real final observation when its character field is
blank or overlaps padding. The driver instead uses the row-aligned 0–79-byte
policy documented in `docs/spec/xpt.md`; dedicated Rust tests exercise blank tails.
ReadStat correctly read the five-row, 20-byte-row variant with an empty final
character field during development (pandas returned only four rows).
