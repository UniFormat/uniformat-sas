"""J6 côté Python : `write_sas`, aller-retour sur le corpus, relecture par pandas et ReadStat, limites, transaction."""

from __future__ import annotations

import os
import warnings

import pyarrow as pa
import pytest

import uniformat_io.sas as sas

from conftest import corpus_files


def table():
    return pa.table(
        {
            "i": pa.array([1, 2, 3, 4], pa.int32()),
            "x": pa.array([1.5, None, 3.0, None]),
            "x__missing": pa.array([0, 3, 0, 28], pa.uint8()),
            "s": pa.array(["a", "bé", None, "texte"]),
            "d": pa.array([0, 1000, None, 20000], pa.date32()),
            "t": pa.array([0, 10**6, None, 86_400 * 10**6], pa.timestamp("us")),
        }
    )


@pytest.mark.parametrize("compression", [None, "char", "binary"])
def test_roundtrip_values_types_and_missing(tmp_path, compression):
    f = tmp_path / "t.sas7bdat"
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", sas.SasWarning)
        report = sas.write_sas(f, table(), compression=compression, formats={"x": "COMMA10.2"}, column_labels={"i": "Entier"})
    assert report["rows"] == 4 and report["truncated_values"] == 0
    t = sas.read_sas_table(f, missing="sidecar")
    assert t.column("i").to_pylist() == [1.0, 2.0, 3.0, 4.0]
    assert t.column("x").to_pylist() == [1.5, None, 3.0, None]
    assert t.column("x__missing").to_pylist() == [0, 3, 0, 28]  # .A et .Z
    assert t.column("s").to_pylist() == ["a", "bé", "", "texte"]
    assert t.column("d").to_pylist()[:2] == table().column("d").to_pylist()[:2]
    md = sas.metadata(f)
    cols = {c["name"]: c for c in md["columns"]}
    assert cols["i"]["label"] == "Entier"
    assert (cols["x"]["format"], cols["x"]["format_width"], cols["x"]["format_decimals"]) == ("COMMA", 10, 2)
    assert md["table"]["compression"] == (compression or "none")


def test_corpus_files_survive_a_rewrite(corpus, tmp_path):
    """Les données d'un fichier du corpus relues, réécrites (plat, CHAR, BINARY) puis relues restent identiques."""
    names = ["base/base_n100_mix_no", "taille/taille_n50000_no", "valeurs/valeurs_manquants_no", "types/types_char200_no",
             "metadonnees/metadonnees_fmt_comma10_2_no", "compression/compression_pat2_char"]
    out = tmp_path / "r.sas7bdat"
    for n in names:
        src = corpus / "files" / (n + ".sas7bdat")
        orig = sas.read_sas_table(src, missing="sidecar")
        for comp in (None, "char", "binary"):
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", sas.SasWarning)
                sas.write_sas(out, orig, compression=comp)
            back = sas.read_sas_table(out, missing="sidecar")
            assert back.num_rows == orig.num_rows, (n, comp)
            for name in orig.schema.names:
                assert back.column(name).to_pylist() == orig.column(name).to_pylist(), (n, comp, name)


def test_pandas_and_polars_dataframes_and_attrs(tmp_path):
    pd = pytest.importorskip("pandas")
    f = tmp_path / "p.sas7bdat"
    src = pd.DataFrame({"a": [1.0, None, 3.0], "b": ["x", "y", None], "c": pd.to_datetime(["2020-01-01", None, "2021-06-30"])})
    sas.write_sas(f, src, formats={"c": "DATETIME20."})
    back = sas.read_sas_pandas(f)
    assert back["a"].isna().tolist() == [False, True, False]
    assert back["b"].tolist()[:2] == ["x", "y"]
    assert str(back["c"].iloc[0])[:10] == "2020-01-01"
    # un DataFrame lu avec des manquants spéciaux les redonne à l'écriture, avec ses labels
    miss = pd.DataFrame({"x": [1.0, None, None]})
    miss.attrs["sas_missing"] = pd.DataFrame({"x": [None, "_", "Z"]})
    sas.write_sas(f, miss)
    again = sas.read_sas_pandas(f, missing="sidecar", keep_missing_columns=True)
    assert again["x__missing"].tolist() == [0, 2, 28]
    pl = pytest.importorskip("polars")
    sas.write_sas(f, pl.DataFrame({"u": [1, 2], "v": ["a", "b"]}))
    assert sas.read_sas_polars(f)["v"].to_list() == ["a", "b"]


def test_readstat_and_pandas_read_the_written_files(tmp_path):
    pyreadstat = pytest.importorskip("pyreadstat")
    pd = pytest.importorskip("pandas")
    f = tmp_path / "w.sas7bdat"
    t = table()
    for comp in (None, "char", "binary"):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", sas.SasWarning)
            sas.write_sas(f, t, compression=comp, formats={"x": "COMMA10.2"}, column_labels={"s": "Texte"})
        df, meta = pyreadstat.read_sas7bdat(str(f))
        assert df.shape[0] == 4 and "Texte" in meta.column_labels
        assert df["i"].tolist() == [1.0, 2.0, 3.0, 4.0]
        assert pd.read_sas(f, format="sas7bdat", encoding="utf-8").shape[0] == 4


def test_rdc_streams_are_readable_by_readstat(tmp_path):
    """Régression J6 : une copie RDC ne doit pas recouvrir sa source (ReadStat la refuse)."""
    pyreadstat = pytest.importorskip("pyreadstat")
    t = pa.table(
        {
            "id": [1.0, 2, 3, 4, 5],
            "x": pa.array([1.5, None, 3.0, None, None]),
            "x__missing": pa.array([0, 1, 0, 3, 28], pa.uint8()),
            "flag": [1.0, 0, None, 1, 1],
            "s": ["a", "bé", "", "", "texte long"],
            "d": [18262.0 + i for i in range(5)],
            "t": [1.0e9 + 3600 * i for i in range(5)],
            "h": [3600.0 * i for i in range(1, 6)],
        }
    )
    f = tmp_path / "rdc.sas7bdat"
    sas.write_sas(f, t, compression="binary")
    df, _ = pyreadstat.read_sas7bdat(str(f))
    assert df.shape == (5, 7)


def test_limits_are_typed_errors_before_any_file(tmp_path):
    f = tmp_path / "x.sas7bdat"
    cases = [
        (pa.table({"n" * 33: [1.0]}), {}),
        (pa.table({"x": [1.0]}), {"column_labels": {"x": "L" * 257}}),
        (pa.table({"c": ["a"]}), {"lengths": {"c": 32768}}),
        (pa.table({"c": ["a"]}), {"lengths": {"c": 0}}),
        (pa.table({"x": [1.0]}), {"lengths": {"x": 2}}),
        (pa.table({"c": ["x" * 20]}), {"lengths": {"c": 10}}),
        (pa.table({"x": [float("inf")]}), {}),
    ]
    for data, kw in cases:
        with pytest.raises(sas.SasWriteError):
            sas.write_sas(f, data, **kw)
        assert not f.exists()
        assert [p.name for p in tmp_path.iterdir()] == []
    # trop de colonnes pour la page demandée, sans agrandissement
    with pytest.raises(sas.SasWriteError):
        sas.write_sas(f, pa.table({f"v{k}": [1.0] for k in range(700)}), grow_page=False)
    assert not f.exists()
    with pytest.raises(ValueError):
        sas.write_sas(f, pa.table({"x": [1.0]}), compression="zip")


def test_unrepresentable_policies(tmp_path):
    f = tmp_path / "u.sas7bdat"
    t2 = pa.table({"c": ["abcdef", "éé"]})
    rep = sas.write_sas(f, t2, lengths={"c": 4}, on_unrepresentable="truncate")
    assert rep["truncated_values"] == 1
    assert sas.read_sas_table(f).column("c").to_pylist() == ["abcd", "éé"]
    # latin1 : le signe euro n'existe pas ; replace le remplace par ?
    rep = sas.write_sas(f, pa.table({"c": ["5€"]}), encoding="latin1", on_unrepresentable="replace")
    assert rep["replaced_chars"] == 1
    assert sas.read_sas_table(f).column("c").to_pylist() == ["5?"]
    with pytest.raises(sas.SasWriteError):
        sas.write_sas(f, pa.table({"c": ["5€"]}), encoding="latin1")
    # infini : remplacé par un manquant
    rep = sas.write_sas(f, pa.table({"x": [1.0, float("inf")]}), on_unrepresentable="replace")
    assert rep["replaced_values"] == 1
    # entier de plus de 53 bits
    big = pa.table({"n": pa.array([2**60 + 1], pa.int64())})
    with pytest.raises(sas.SasWriteError):
        sas.write_sas(f, big)
    assert sas.write_sas(f, big, on_unrepresentable="replace")["inexact_integers"] == 1


def test_write_is_transactional_and_atomic(tmp_path):
    f = tmp_path / "a.sas7bdat"
    sas.write_sas(f, pa.table({"x": [1.0, 2.0]}))
    before = f.read_bytes()
    with pytest.raises(sas.SasWriteError):
        sas.write_sas(f, pa.table({"c": ["x" * 50]}), lengths={"c": 10})
    assert f.read_bytes() == before, "le fichier existant n'est pas touché par une écriture qui échoue"
    assert [p.name for p in tmp_path.iterdir()] == ["a.sas7bdat"]
    # dossier absent : OSError, aucun fichier
    with pytest.raises(OSError):
        sas.write_sas(tmp_path / "nope" / "a.sas7bdat", pa.table({"x": [1.0]}))


def test_reproducible_output(tmp_path):
    a, b = tmp_path / "a.sas7bdat", tmp_path / "b.sas7bdat"
    for p in (a, b):
        sas.write_sas(p, table(), file_code=12345, created=1_700_000_000.0)
    assert a.read_bytes() == b.read_bytes()


def test_small_tables_are_not_compressed_with_a_warning(tmp_path):
    f = tmp_path / "s.sas7bdat"
    with pytest.warns(sas.SasWarning, match="compression_disabled_small_rows"):
        sas.write_sas(f, pa.table({"x": [1.0]}), compression="char")
    assert sas.metadata(f)["table"]["compression"] == "none"
