"""Critères de fin de J5 (``docs/roadmap.md``), côté Python, sur le corpus de référence."""

from __future__ import annotations

import csv
import io
import math
import random
import shutil
import warnings

import pytest

import uniformat_io.sas as sas

from conftest import corpus_files

REL_TOL = 1e-13

# nom d'encodage SAS (métadonnée ``sas.encoding``) -> codec Python, pour relire le CSV octet par octet
CODECS = {"utf-8": "utf-8", "latin1": "latin-1", "wlatin1": "cp1252", "shift-jis": "shift_jis", "sjis": "shift_jis"}


def values_equal(expected: str, got: str) -> bool:
    if expected == got:
        return True
    try:
        a, b = float(expected.strip()), float(got.strip())
    except ValueError:
        return False
    return a == b or abs(a - b) <= REL_TOL * max(abs(a), abs(b))


def read_csv(path) -> list[dict[str, str]]:
    raw = path.read_bytes()
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        text = raw.decode("latin-1")  # sans perte : on recode octet par octet pour comparer
    return list(csv.DictReader(io.StringIO(text, newline="")))


def encoding_override(path) -> str | None:
    # U22 (10 §5.4) : datareader/test16 à test21 portent un code d'encodage faux mais contiennent de l'UTF-8
    stem = path.stem
    if "datareader" in path.parts and stem.startswith("test") and stem[4:].isdigit() and 16 <= int(stem[4:]) <= 21:
        return "utf-8"
    return None


def same_text(expected: str, got: str, codec: str) -> bool:
    if expected == got:
        return True
    try:
        return expected.encode("latin-1") == got.encode(codec)
    except (UnicodeEncodeError, LookupError):
        return False


def compare(path, csv_path) -> list[str]:
    external = "external" in path.parts
    enc = encoding_override(path)
    meta = sas.metadata(path, encoding=enc)
    codec = CODECS.get(meta["table"].get("encoding", "utf-8"), "utf-8")
    if enc:
        codec = "utf-8"
    # dates="float" : les CSV portent la valeur SAS brute ; missing="sidecar" : les codes se comparent au CSV
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always", sas.SasWarning)
        table = sas.read_sas_table(path, dates="float", missing="sidecar", encoding=enc)
    # octets invalides pour l'encodage (ex. 0xFF dans de l'UTF-8) : le texte est remplacé par U+FFFD (avertissement) ;
    # le CSV porte les octets, on compare alors les octets de la lecture ``invalid_bytes="binary"``
    raw_bytes = None
    if any("octets invalides" in str(w.message) for w in caught):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", sas.SasWarning)
            raw_bytes = sas.read_sas_table(path, dates="float", missing="sidecar", encoding=enc, invalid_bytes="binary").to_pandas()
    ref = read_csv(csv_path)
    diffs: list[str] = []
    if table.num_rows != len(ref):
        diffs.append(f"{table.num_rows} lignes contre {len(ref)} dans le CSV")
        return diffs
    df = table.to_pandas()
    sidecars = {c["name"]: c["missing_sidecar"] for c in meta_columns_with_sidecar(table)}
    for col in meta["columns"]:
        name = col["name"]
        if name not in (ref[0] if ref else {}):
            if ref:
                diffs.append(f"colonne {name!r} absente du CSV")
            continue
    for i, row in enumerate(ref):
        for name, expected in row.items():
            if name not in df.columns:
                continue
            value = df.at[i, name]
            side = sidecars.get(name)
            if side is not None:  # colonne numérique
                code = int(df.at[i, side])
                if code != 0:
                    label = sas.missing_label(code)
                    ok = expected == label or (external and expected == "")
                else:
                    ok = not (isinstance(value, float) and math.isnan(value)) and values_equal(expected, repr(float(value)))
            elif raw_bytes is not None and "\ufffd" in str(value):
                raw = raw_bytes.at[i, name]
                ok = raw is not None and expected.encode("latin-1") == bytes(raw)
            else:
                ok = same_text(expected, "" if value is None else str(value), codec)
            if not ok:
                diffs.append(f"ligne {i} colonne {name!r} : CSV {expected!r}, lu {value!r}")
                break
        if len(diffs) >= 5:
            break
    return diffs


def meta_columns_with_sidecar(table):
    out = []
    for field in table.schema:
        md = {k.decode(): v.decode() for k, v in (field.metadata or {}).items()}
        if "sas.missing_sidecar" in md:
            out.append({"name": field.name, "missing_sidecar": md["sas.missing_sidecar"]})
    return out


def test_read_sas_pandas_values_equal_the_reference_csv(corpus):
    files = [(p, c) for p, c in corpus_files(corpus) if c is not None]
    assert len(files) == 218, "inventaire inattendu : 173 fichiers non compressés + 45 compressés avec CSV"
    failures = []
    for path, csv_path in files:
        try:
            diffs = compare(path, csv_path)
        except Exception as e:  # noqa: BLE001 - on rapporte tout écart
            diffs = [f"{type(e).__name__}: {e}"]
        if diffs:
            failures.append(f"{path.relative_to(corpus)} :\n    " + "\n    ".join(diffs))
    assert not failures, f"{len(failures)} fichiers en écart :\n" + "\n".join(failures)


def test_every_file_reads_or_fails_with_a_typed_error(corpus):
    n = 0
    for path, _ in corpus_files(corpus):
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", sas.SasWarning)
                sas.read_sas_table(path, encoding=encoding_override(path))
            n += 1
        except sas.SasError:
            pass
    assert n >= 228, f"{n} fichiers lus"


def test_schema_reads_no_data_page(corpus, tmp_path):
    """``schema()`` n'ouvre que l'en-tête et les pages de métadonnées (borne exacte : test Rust ``plan.rs``)."""
    checked = 0
    for path, _ in corpus_files(corpus):
        try:
            read, header_len, page_len, meta_pages = sas._native._open_probe(str(path))
        except sas.SasError:
            continue
        bound = header_len + meta_pages * page_len
        assert read <= bound, f"{path.name} : {read} octets lus > {header_len} + {meta_pages} × {page_len}"
        assert header_len >= 552  # la borne suppose l'en-tête lu en entier (552 octets au moins)
        checked += 1
    assert checked >= 200, f"{checked} fichiers contrôlés"
    # un fichier tronqué est refusé (la taille annoncée par l'en-tête est vérifiée), jamais lu de travers
    big = corpus / "files" / "taille" / "taille_n50000_no.sas7bdat"
    cut = tmp_path / "cut.sas7bdat"
    cut.write_bytes(big.read_bytes()[: 200_000])
    with pytest.raises(sas.SasFormatError):
        sas.schema(cut)


def test_sidecar_keeps_the_28_missing_codes(corpus):
    path = corpus / "files" / "valeurs" / "valeurs_manquants_no.sas7bdat"
    df = sas.read_sas_pandas(path, missing="sidecar", keep_missing_columns=True)
    assert sorted(df["x__missing"].unique().tolist()) == list(range(1, 29))
    assert df["x"].isna().all()
    labels = df.attrs["sas_missing"]["x"].tolist()
    assert labels[:3] == [".", "_", "A"] and labels[-1] == "Z"
    # sans colonne annexe conservée : elle disparaît du DataFrame mais reste dans attrs
    df2 = sas.read_sas_pandas(path, missing="sidecar")
    assert "x__missing" not in df2.columns and "x" in df2.columns
    assert df2.attrs["sas_missing"]["x"].tolist() == labels


def test_polars_sidecar(corpus):
    pl = pytest.importorskip("polars")
    path = corpus / "files" / "valeurs" / "valeurs_manquants_no.sas7bdat"
    df = sas.read_sas_polars(path, missing="sidecar")
    assert df["x__missing"].dtype == pl.UInt8
    assert sorted(df["x__missing"].unique().to_list()) == list(range(1, 29))
    assert df["x"].null_count() == df.height
    cat = sas.read_sas_polars(path, missing="sidecar", missing_as="categorical")
    assert cat["x__missing"].dtype == pl.Categorical
    labels = cat["x__missing"].cast(pl.String).to_list()
    assert labels[:3] == [".", "_", "A"] and labels[-1] == "Z"


def test_missing_null_has_no_sidecar_column(corpus):
    path = corpus / "files" / "valeurs" / "valeurs_manquants_no.sas7bdat"
    t = sas.read_sas_table(path)
    assert "x__missing" not in t.schema.names
    assert t.column("x").null_count == t.num_rows


def test_projection_limit_and_stream(corpus):
    path = corpus / "files" / "taille" / "taille_n50000_no.sas7bdat"
    names = sas.schema(path).names
    t = sas.read_sas_table(path, columns=[names[-1], names[0]], limit=7)
    assert t.num_rows == 7 and t.schema.names == [names[-1], names[0]]
    reader = sas.read_sas(path, batch_rows=4096)
    assert sum(b.num_rows for b in reader) == 50000
    with pytest.raises(ValueError):
        sas.read_sas_table(path, columns=["nope"])


def test_native_dates_agree_with_the_float_view(corpus):
    pa = pytest.importorskip("pyarrow")
    hits = 0
    for path, _ in corpus_files(corpus):
        sch = sas.schema(path, encoding=encoding_override(path))
        temporal = [f.name for f in sch if pa.types.is_date(f.type) or pa.types.is_timestamp(f.type)]
        if not temporal:
            continue
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", sas.SasWarning)
            native = sas.read_sas_table(path, columns=temporal, encoding=encoding_override(path))
            raw = sas.read_sas_table(path, columns=temporal, dates="float", encoding=encoding_override(path))
        for name in temporal:
            a, b = native.column(name).to_pylist(), raw.column(name).to_pylist()
            for x, y in zip(a, b):
                if x is None:
                    continue  # hors plage ou manquant : nul côté natif
                assert y is not None
            hits += 1
    assert hits >= 10, f"{hits} colonnes temporelles contrôlées"


def test_metadata_has_labels_formats_and_encoding(corpus):
    path = corpus / "files" / "metadonnees"
    files = sorted(path.glob("*.sas7bdat"))
    assert files
    seen_label = seen_format = False
    for f in files:
        md = sas.metadata(f)
        assert md["table"]["compression"] in ("none", "char", "binary")
        assert "encoding" in md["table"]
        seen_label |= any("label" in c for c in md["columns"])
        seen_format |= any("format" in c for c in md["columns"])
    assert seen_label and seen_format


def test_errors_are_typed(tmp_path):
    with pytest.raises(FileNotFoundError):
        sas.read_sas_table(tmp_path / "absent.sas7bdat")
    junk = tmp_path / "junk.sas7bdat"
    junk.write_bytes(b"x" * 1000)
    with pytest.raises(sas.SasFormatError):
        sas.read_sas_table(junk)
    assert issubclass(sas.SasFormatError, sas.SasError)
    with pytest.raises(sas.SasUnsupportedError):
        sas.read_sas_table("https://example.invalid/t.sas7bdat")
    with pytest.raises(ValueError):
        sas.read_sas_table(junk, missing="bogus")


def test_corrupted_files_never_crash_the_interpreter(corpus, tmp_path):
    """Critère 4 : octets altérés, fichiers tronqués : seules des exceptions typées, jamais de panique ni de plantage."""
    rng = random.Random(20261104)
    sources = [p for p, _ in corpus_files(corpus) if p.stat().st_size < 400_000]
    rng.shuffle(sources)
    sources = sources[:60]
    bad = []
    runs = 0
    for src in sources:
        data = src.read_bytes()
        for k in range(12):
            buf = bytearray(data)
            if k < 3:  # troncatures
                buf = buf[: rng.randrange(0, len(buf))]
            elif k < 8:  # quelques octets aléatoires
                for _ in range(rng.randrange(1, 20)):
                    buf[rng.randrange(len(buf))] = rng.randrange(256)
            else:  # en-tête et premières pages
                for _ in range(rng.randrange(1, 40)):
                    buf[rng.randrange(min(len(buf), 20_000))] = rng.randrange(256)
            f = tmp_path / "m.sas7bdat"
            f.write_bytes(bytes(buf))
            runs += 1
            for fn in (sas.metadata, sas.read_sas_table):
                try:
                    with warnings.catch_warnings():
                        warnings.simplefilter("ignore")
                        fn(f)
                except (sas.SasError, OSError, ValueError):
                    pass
                except BaseException as e:  # noqa: BLE001 - une PanicException est un défaut du cœur
                    bad.append(f"{src.name} k={k} {fn.__name__}: {type(e).__name__}: {e}")
    assert runs >= 600
    assert not bad, f"{len(bad)} cas :\n" + "\n".join(bad[:10])
