"""Read and write SAS ``.sas7bdat`` files (Rust driver): Arrow streams, pandas, polars.

The native module ``_native`` produces Arrow streams (PyCapsule interface); this package adapts them to pyarrow,
pandas and polars (ADR 0003, "Python rendering"). Importing it does not register anything: the format is exposed as
:data:`FORMAT` and discovered by ``uniformat_io.core`` through the ``uniformat_io.formats`` entry point.
"""

from __future__ import annotations

import os
import re
from typing import Any, Dict, Iterable, List, Optional, Union

from . import _native
from ._native import (
    SasError,
    SasFormatError,
    SasUnsupportedError,
    SasWarning,
    SasWriteError,
)

__all__ = [
    "FORMAT",
    "SasError",
    "SasFormatError",
    "SasUnsupportedError",
    "SasWarning",
    "SasWriteError",
    "metadata",
    "missing_label",
    "read_sas",
    "read_sas_pandas",
    "read_sas_polars",
    "read_sas_table",
    "schema",
    "write_sas",
]

PathLike = Union[str, "os.PathLike[str]"]

#: Libellé SAS d'un code de la colonne annexe ``x__missing`` (ADR 0003) : 1 = ``.``, 2 = ``_``, 3 à 28 = ``A`` à ``Z``.
_MISSING_LABELS: Dict[int, str] = {1: ".", 2: "_", **{3 + i: chr(ord("A") + i) for i in range(26)}}


def missing_label(code: int) -> Optional[str]:
    """Libellé du manquant spécial pour un code de colonne annexe (``None`` si la valeur est présente)."""
    if code == 0:
        return None
    try:
        return _MISSING_LABELS[code]
    except KeyError:
        raise ValueError(f"code de manquant inconnu : {code}") from None


def _pa():
    import pyarrow as pa

    return pa


def _opts(kw: Dict[str, Any]) -> Dict[str, Any]:
    return {k: v for k, v in kw.items() if v is not None or k in ("columns", "limit", "encoding")}


def read_sas(
    path: PathLike,
    *,
    columns: Optional[Iterable[str]] = None,
    limit: Optional[int] = None,
    trailing_blanks: str = "trim",
    missing: str = "null",
    missing_suffix: str = "__missing",
    dates: str = "native",
    timestamp_unit: str = "us",
    time: str = "time",
    out_of_range: str = "null",
    invalid_bytes: str = "replace",
    empty_as_null: bool = False,
    encoding: Optional[str] = None,
    strict: bool = False,
    batch_rows: int = 8192,
    plan: str = "fast",
):
    """Flux de lots Arrow (``pyarrow.RecordBatchReader``) ; la lecture est paresseuse.

    Les avertissements de lecture (valeurs rendues nulles, octets remplacés) ne sont émis que par
    :func:`read_sas_table` et ses dérivés ; ceux de l'ouverture le sont ici.
    """
    native = _native.read_sas(
        os.fspath(path),
        columns=None if columns is None else list(columns),
        limit=limit,
        trailing_blanks=trailing_blanks,
        missing=missing,
        missing_suffix=missing_suffix,
        dates=dates,
        timestamp_unit=timestamp_unit,
        time=time,
        out_of_range=out_of_range,
        invalid_bytes=invalid_bytes,
        empty_as_null=empty_as_null,
        encoding=encoding,
        strict=strict,
        batch_rows=batch_rows,
        plan=plan,
    )
    return _pa().RecordBatchReader.from_stream(native)


def read_sas_table(
    path: PathLike,
    *,
    columns: Optional[Iterable[str]] = None,
    limit: Optional[int] = None,
    trailing_blanks: str = "trim",
    missing: str = "null",
    missing_suffix: str = "__missing",
    dates: str = "native",
    timestamp_unit: str = "us",
    time: str = "time",
    out_of_range: str = "null",
    invalid_bytes: str = "replace",
    empty_as_null: bool = False,
    encoding: Optional[str] = None,
    strict: bool = False,
    batch_rows: int = 8192,
    plan: str = "fast",
):
    """Table Arrow (``pyarrow.Table``) ; émet les avertissements de lecture (``SasWarning``)."""
    native = _native.read_table(
        os.fspath(path),
        columns=None if columns is None else list(columns),
        limit=limit,
        trailing_blanks=trailing_blanks,
        missing=missing,
        missing_suffix=missing_suffix,
        dates=dates,
        timestamp_unit=timestamp_unit,
        time=time,
        out_of_range=out_of_range,
        invalid_bytes=invalid_bytes,
        empty_as_null=empty_as_null,
        encoding=encoding,
        strict=strict,
        batch_rows=batch_rows,
        plan=plan,
    )
    return _pa().table(native)


def schema(
    path: PathLike,
    *,
    missing: str = "null",
    missing_suffix: str = "__missing",
    dates: str = "native",
    timestamp_unit: str = "us",
    time: str = "time",
    invalid_bytes: str = "replace",
    encoding: Optional[str] = None,
    strict: bool = False,
):
    """Schéma Arrow (``pyarrow.Schema``) avec les métadonnées ``sas.*`` ; aucune page de données n'est lue."""
    native = _native.schema(
        os.fspath(path),
        missing=missing,
        missing_suffix=missing_suffix,
        dates=dates,
        timestamp_unit=timestamp_unit,
        time=time,
        invalid_bytes=invalid_bytes,
        encoding=encoding,
        strict=strict,
    )
    return _pa().schema(native)


_TABLE_INTS = ("sas.encoding_code", "sas.row_count", "sas.deleted_count")


def _decode(meta: Optional[Dict[bytes, bytes]]) -> Dict[str, str]:
    return {k.decode(): v.decode() for k, v in (meta or {}).items()}


def metadata_from_schema(sch) -> Dict[str, Any]:
    """Métadonnées SAS d'un schéma Arrow (``schema()`` ou ``Table.schema``) sous forme de dictionnaire."""
    table_md = _decode(sch.metadata)
    table: Dict[str, Any] = {}
    for key, value in table_md.items():
        if not key.startswith("sas."):
            continue
        name = key[len("sas.") :]
        table[name] = int(value) if key in _TABLE_INTS else value
    columns: List[Dict[str, Any]] = []
    for i, field in enumerate(sch):
        fmd = _decode(field.metadata)
        col: Dict[str, Any] = {"index": i, "name": field.name, "arrow_type": str(field.type)}
        for key, value in fmd.items():
            if not key.startswith("sas."):
                continue
            name = key[len("sas.") :]
            if name in ("format_width", "format_decimals", "length"):
                col[name] = int(value)
            elif name == "index":
                col[name] = value == "true"
            else:
                col[name] = value
        columns.append(col)
    return {"table": table, "columns": columns}


def metadata(path: PathLike, **kw: Any) -> Dict[str, Any]:
    """Métadonnées de la table : ``{"table": {...}, "columns": [...]}`` (sans lecture de page de données)."""
    return metadata_from_schema(schema(path, **kw))


def _sidecars(sch) -> Dict[str, str]:
    """Colonne numérique -> colonne annexe, d'après la métadonnée ``sas.missing_sidecar``."""
    out: Dict[str, str] = {}
    for field in sch:
        fmd = _decode(field.metadata)
        if "sas.missing_sidecar" in fmd:
            out[field.name] = fmd["sas.missing_sidecar"]
    return out


def read_sas_pandas(
    path: PathLike,
    *,
    keep_missing_columns: bool = False,
    **kw: Any,
):
    """``pandas.DataFrame`` ; ``df.attrs["sas"]`` porte les métadonnées SAS.

    Avec ``missing="sidecar"`` (ADR 0003) : la colonne ``x`` reste ``float64`` avec ``NaN`` ; les codes se lisent dans
    ``df.attrs["sas_missing"]`` (nom de colonne -> ``Series`` de ``str`` : ``"."``, ``"_"``, ``"A"``…, ou ``None``),
    ou dans les colonnes ``x__missing`` conservées avec ``keep_missing_columns=True``. pandas n'a pas de type
    « NA étiqueté » : la restitution ne peut pas être plus fidèle que la colonne annexe.
    """
    import pandas as pd

    table = read_sas_table(path, **kw)
    sidecars = _sidecars(table.schema)
    meta = metadata_from_schema(table.schema)
    drop = [] if keep_missing_columns else list(sidecars.values())
    df = table.to_pandas(date_as_object=False)
    if sidecars:
        labels = {}
        for col, side in sidecars.items():
            codes = df[side]
            labels[col] = codes.map(lambda c: missing_label(int(c)))
        df.attrs["sas_missing"] = pd.DataFrame(labels, index=df.index)
    if drop:
        df = df.drop(columns=drop)
    df.attrs["sas"] = meta
    return df


def read_sas_polars(
    path: PathLike,
    *,
    missing_as: str = "uint8",
    **kw: Any,
):
    """``polars.DataFrame`` ; avec ``missing="sidecar"``, ``x`` est ``Float64`` nulle et ``x__missing`` est ``UInt8``
    (ou ``Categorical`` de ``"."``, ``"_"``, ``"A"``… avec ``missing_as="categorical"``).
    """
    import polars as pl

    if missing_as not in ("uint8", "categorical"):
        raise ValueError(f"missing_as = {missing_as!r} : valeurs admises uint8, categorical")
    table = read_sas_table(path, **kw)
    sidecars = _sidecars(table.schema)
    df = pl.from_arrow(table)
    if missing_as == "categorical":
        for side in sidecars.values():
            df = df.with_columns(
                pl.col(side)
                .replace_strict(_MISSING_LABELS, default=None, return_dtype=pl.String)
                .cast(pl.Categorical)
                .alias(side)
            )
    return df


_FORMAT_RE = re.compile(r"^(?P<name>\$?[A-Za-z_]*?)(?P<width>\d*)(?:\.(?P<dec>\d*))?$")


def _parse_format(spec):
    """``"DATE9."``, ``"COMMA10.2"``, ``"$CHAR20."`` ou ``("DATE", 9, 0)`` -> ``(nom, largeur, décimales)``."""
    if isinstance(spec, (tuple, list)):
        name, width, dec = (list(spec) + [0, 0])[:3]
        return str(name), int(width), int(dec)
    m = _FORMAT_RE.match(str(spec).strip())
    if not m:
        raise ValueError(f"format SAS illisible : {spec!r}")
    return m.group("name"), int(m.group("width") or 0), int(m.group("dec") or 0)


def _to_arrow(data):
    """Table Arrow depuis un objet Arrow (``__arrow_c_stream__``), un DataFrame pandas ou polars."""
    pa = _pa()
    if hasattr(data, "to_arrow") and not hasattr(data, "__arrow_c_stream__"):
        data = data.to_arrow()
    if hasattr(data, "__arrow_c_stream__"):
        return pa.table(data)
    if hasattr(data, "columns") and hasattr(data, "dtypes"):  # pandas
        return pa.Table.from_pandas(data, preserve_index=False)
    raise TypeError(f"type de données non pris en charge : {type(data).__name__}")


def write_sas(
    path: PathLike,
    data,
    *,
    compression: Optional[str] = None,
    encoding: str = "utf-8",
    column_labels: Optional[Dict[str, str]] = None,
    formats: Optional[Dict[str, Any]] = None,
    informats: Optional[Dict[str, Any]] = None,
    lengths: Optional[Dict[str, int]] = None,
    page_length: int = 65536,
    grow_page: bool = True,
    on_unrepresentable: str = "error",
    missing_suffix: str = "__missing",
    table_name: Optional[str] = None,
    file_code: Optional[int] = None,
    created: Optional[float] = None,
) -> Dict[str, Any]:
    """Écrit ``data`` (table Arrow, ``RecordBatchReader``, DataFrame pandas ou polars) dans un ``.sas7bdat``.

    - ``compression`` : ``None`` (défaut), ``"char"`` ou ``"binary"`` ; une table de moins de 16 octets par ligne
      reste non compressée (avertissement).
    - ``encoding`` : ``"utf-8"``, ``"latin1"`` ou ``"wlatin1"`` (cp1252).
    - ``column_labels``, ``formats`` (``"DATE9."``, ``"COMMA10.2"`` ou ``("DATE", 9, 0)``), ``informats``,
      ``lengths`` (octets : 3 à 8 pour un numérique, 1 à 32 767 pour un caractère) : par nom de colonne. Sans
      ``lengths``, les colonnes caractère sont mesurées sur les données. Un DataFrame lu par :func:`read_sas_pandas`
      redonne ses labels et formats (``df.attrs["sas"]``) et ses manquants spéciaux (``df.attrs["sas_missing"]``).
    - Une colonne ``x__missing`` (``uint8``, ADR 0003) à côté de ``x`` donne le code de manquant spécial
      (1 = ``.``, 2 = ``_``, 3 à 28 = ``A`` à ``Z``).
    - ``on_unrepresentable`` : ``"error"`` (défaut), ``"replace"`` ou ``"truncate"`` (ADR 0005 §4).
    - ``page_length`` : taille de page demandée ; ``grow_page=True`` l'agrandit si les métadonnées ou une ligne
      n'y tiennent pas (``SasWriteError`` sinon).
    - ``file_code`` et ``created`` (secondes Unix) rendent le fichier reproductible.

    L'écriture est transactionnelle : aucun fichier n'est laissé en cas d'erreur. Rend le rapport (dictionnaire).
    """
    pa = _pa()
    attrs = getattr(data, "attrs", None) or {}
    table = _to_arrow(data)
    meta = attrs.get("sas") if isinstance(attrs, dict) else None
    missing_df = attrs.get("sas_missing") if isinstance(attrs, dict) else None
    labels = dict(column_labels or {})
    fmts = dict(formats or {})
    infmts = dict(informats or {})
    lens = dict(lengths or {})
    if meta:
        for c in meta.get("columns", []):
            name = c.get("name")
            if name not in table.schema.names:
                continue
            if "label" in c:
                labels.setdefault(name, c["label"])
            if "format" in c:
                fmts.setdefault(name, (c["format"], c.get("format_width", 0), c.get("format_decimals", 0)))
            if "informat" in c:
                infmts.setdefault(name, c["informat"])
    # manquants spéciaux d'un DataFrame lu avec missing="sidecar"
    if missing_df is not None:
        inverse = {v: k for k, v in _MISSING_LABELS.items()}
        for col in missing_df.columns:
            side = col + missing_suffix
            if col in table.schema.names and side not in table.schema.names:
                codes = [0 if v is None or v != v else inverse[v] for v in missing_df[col].tolist()]
                table = table.append_column(side, pa.array(codes, type=pa.uint8()))
    fields = []
    for f in table.schema:
        md = dict(f.metadata or {})
        enc = lambda v: str(v).encode()  # noqa: E731
        if f.name in labels:
            md[b"sas.label"] = enc(labels[f.name])
        if f.name in fmts:
            n, w, d = _parse_format(fmts[f.name])
            md[b"sas.format"], md[b"sas.format_width"], md[b"sas.format_decimals"] = enc(n), enc(w), enc(d)
        if f.name in infmts:
            n, w, d = _parse_format(infmts[f.name])
            md[b"sas.informat"], md[b"sas.informat_width"], md[b"sas.informat_decimals"] = enc(n), enc(w), enc(d)
        if f.name in lens:
            md[b"sas.length"] = enc(lens[f.name])
        fields.append(f.with_metadata(md))
    table = table.cast(pa.schema(fields, metadata=table.schema.metadata)) if False else pa.Table.from_arrays(
        table.columns, schema=pa.schema(fields, metadata=table.schema.metadata)
    )
    return _native.write_table(
        os.fspath(path),
        table,
        compression=compression or "none",
        encoding=encoding,
        page_length=page_length,
        grow_page=grow_page,
        on_unrepresentable=on_unrepresentable,
        missing_suffix=missing_suffix,
        table_name=table_name,
        file_code=file_code,
        created=created,
    )


def _core_read(path, **options):
    return read_sas_table(path, **options)


def _core_write(path, data, **options):
    return write_sas(path, data, **options)


def _make_format():
    from uniformat_io.core import Format

    return Format(
        name="sas7bdat",
        extensions=("sas7bdat",),
        read_table=_core_read,
        write_table=_core_write,
        schema=schema,
        metadata=metadata,
        description="SAS data sets (.sas7bdat), plain or compressed (RLE, RDC)",
    )


#: Format description registered under the ``uniformat_io.formats`` entry-point group.
FORMAT = _make_format()
