#!/usr/bin/env python3
"""Check the asset format draft in docs/asset-format/: schemas, examples and links.

* every schema in schemas/ (and schemas/gltf/) is a valid JSON Schema 2020-12 document;
* every TOML example in examples/ validates against the schema its `schema = "ob.<kind>/<v>"` line names
  (schemas/<kind>.schema.json); a `pack.toml` validates against schemas/pack.schema.json;
* every glTF example: each `OB_*` extension object validates against schemas/gltf/OB_*.schema.json, every
  extension used is declared in `extensionsUsed`, each buffer's file exists with the declared `byteLength`, and every
  buffer view and accessor lies inside its buffer;
* every relative Markdown link between the pages (and to schemas/ and examples/) points at an existing file.

Usage: scripts/docs/asset_format_check.py [DIR]   (default docs/asset-format next to this script's repository)
Exit 0 when everything passes, 1 on any failure, 3 when the Python package `jsonschema` is missing (the schema
checks cannot run; install it with `python3 -m pip install jsonschema`).
"""
import json
import re
import sys
import tomllib
from pathlib import Path

try:
    import jsonschema
    from referencing import Registry, Resource
    from referencing.jsonschema import DRAFT202012
except ImportError:
    print("SKIP: the Python package jsonschema is not installed (python3 -m pip install jsonschema)")
    sys.exit(3)

ROOT = Path(__file__).resolve().parents[2]


def load_registry(schema_dir):
    schemas, errors = {}, []
    for path in sorted(schema_dir.rglob("*.schema.json")):
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator.check_schema(doc)
        except Exception as e:  # noqa: BLE001 - report every kind of failure the same way
            errors.append(f"{path.relative_to(schema_dir)}: invalid schema: {e}")
            continue
        schemas[path.relative_to(schema_dir).as_posix()] = doc
    resources = []
    for rel, doc in schemas.items():
        res = Resource.from_contents(doc, default_specification=DRAFT202012)
        resources.append((doc.get("$id", rel), res))
        resources.append((rel, res))
    return schemas, Registry().with_resources(resources), errors


def validate(doc, schema, registry):
    validator = jsonschema.Draft202012Validator(schema, registry=registry)
    return sorted(validator.iter_errors(doc), key=lambda e: list(e.absolute_path))


def fmt(err):
    where = "/".join(str(p) for p in err.absolute_path) or "(root)"
    return f"{where}: {err.message}"


def check_toml(path, schemas, registry, base):
    rel = path.relative_to(base)
    try:
        doc = tomllib.loads(path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        return [f"{rel}: not valid TOML: {e}"]
    if path.name == "pack.toml":
        key = "pack.schema.json"
    else:
        m = re.fullmatch(r"ob\.([a-z0-9-]+)/[0-9]+\.[0-9]+", str(doc.get("schema", "")))
        if not m:
            return [f"{rel}: no `schema = \"ob.<kind>/<major>.<minor>\"` line"]
        key = f"{m.group(1)}.schema.json"
    if key not in schemas:
        return [f"{rel}: no schema {key}"]
    return [f"{rel}: {fmt(e)}" for e in validate(doc, schemas[key], registry)]


def walk_extensions(node, found):
    if isinstance(node, dict):
        for name, value in (node.get("extensions") or {}).items():
            found.append((name, value))
        for value in node.values():
            walk_extensions(value, found)
    elif isinstance(node, list):
        for value in node:
            walk_extensions(value, found)


def check_gltf(path, schemas, registry, base):
    rel = path.relative_to(base)
    out = []
    doc = json.loads(path.read_text(encoding="utf-8"))
    used = set(doc.get("extensionsUsed", []))
    if not set(doc.get("extensionsRequired", [])) <= used:
        out.append(f"{rel}: extensionsRequired is not a subset of extensionsUsed")
    found = []
    walk_extensions({k: v for k, v in doc.items() if k != "extensions"}, found)
    for name, value in found:
        if name not in used:
            out.append(f"{rel}: extension {name} is used but not in extensionsUsed")
        key = f"gltf/{name}.schema.json"
        if name.startswith("OB_"):
            if key not in schemas:
                out.append(f"{rel}: no schema {key}")
                continue
            out += [f"{rel}: {name}: {fmt(e)}" for e in validate(value, schemas[key], registry)]
    sizes = []
    for i, buf in enumerate(doc.get("buffers", [])):
        file = path.parent / buf.get("uri", "")
        if not file.is_file():
            out.append(f"{rel}: buffer {i}: missing file {buf.get('uri')}")
            sizes.append(None)
            continue
        size = file.stat().st_size
        if size != buf.get("byteLength"):
            out.append(f"{rel}: buffer {i}: {size} bytes on disk, byteLength {buf.get('byteLength')}")
        sizes.append(size)
    views = doc.get("bufferViews", [])
    for i, view in enumerate(views):
        size = sizes[view["buffer"]] if view["buffer"] < len(sizes) else None
        end = view.get("byteOffset", 0) + view["byteLength"]
        if size is not None and end > size:
            out.append(f"{rel}: bufferView {i} ends at {end}, past its buffer ({size} bytes)")
    comp = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
    width = {"SCALAR": 1, **{f"VEC{n}": n for n in (2, 3, 4)}, **{f"MAT{n}": n * n for n in (2, 3, 4)}}
    for i, acc in enumerate(doc.get("accessors", [])):
        if "bufferView" not in acc:
            continue
        view = views[acc["bufferView"]]
        elem = comp[acc["componentType"]] * width[acc["type"]]
        stride = view.get("byteStride", elem)
        end = acc.get("byteOffset", 0) + stride * (acc["count"] - 1) + elem
        if end > view["byteLength"]:
            out.append(f"{rel}: accessor {i} ends at {end}, past its buffer view ({view['byteLength']} bytes)")
    return out


LINK = re.compile(r"\]\(([^)#\s]+)(?:#[^)]*)?\)")


def check_links(base):
    out = []
    for page in sorted(base.rglob("*.md")):
        for n, line in enumerate(page.read_text(encoding="utf-8").splitlines(), 1):
            for target in LINK.findall(line):
                if "://" in target or target.startswith("mailto:"):
                    continue
                if not (page.parent / target).exists():
                    out.append(f"{page.relative_to(base)}:{n}: broken link {target}")
    return out


def main():
    base = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "docs" / "asset-format"
    schemas, registry, failures = load_registry(base / "schemas")
    counts = {"schemas": len(schemas), "toml": 0, "gltf": 0}
    for path in sorted((base / "examples").rglob("*")):
        if path.suffix == ".toml":
            counts["toml"] += 1
            failures += check_toml(path, schemas, registry, base)
        elif path.suffix == ".gltf":
            counts["gltf"] += 1
            failures += check_gltf(path, schemas, registry, base)
    failures += check_links(base)
    for line in failures:
        print("FAIL", line)
    status = "FAIL" if failures else "ok"
    print(f"ASSET FORMAT: {status} ({counts['schemas']} schemas, {counts['toml']} TOML examples, "
          f"{counts['gltf']} glTF examples, {len(failures)} failures)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
