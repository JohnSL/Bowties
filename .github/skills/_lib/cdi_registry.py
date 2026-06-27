"""Shared CDI XML registry for profile-extraction skills.

Parses an LCC node's CDI XML into a tree of `CdiNode` objects, supports
path lookups that tolerate the various notations the extraction files use
(literal `/` inside element names, `[N]` / `[N-M]` index disambiguators,
and `<repname>` collapse for replicated groups), and computes the
canonical path string each skill should emit for skeletons.

This module is imported by `profile_tools.py`; it is not meant to be run
directly. Keep it dependency-free (stdlib only) so the registry is cheap
to construct and easy to reason about.
"""
from __future__ import annotations

import re
import xml.etree.ElementTree as ET
from collections.abc import Iterator
from dataclasses import dataclass, field
from pathlib import Path

LEAF_KINDS = {"int", "string", "float", "bit"}
EVENTID_KIND = "eventid"

INDEX_SUFFIX_RE = re.compile(r"\[(\d+)(?:-(\d+))?\]$")


@dataclass
class CdiNode:
    """One element in the CDI tree (root, segment, group, leaf, or eventid)."""

    name: str
    kind: str  # "root" | "segment" | "group" | "leaf" | "eventid"
    repname: str | None = None
    replication: int = 1
    enum_map: dict[int, str] = field(default_factory=dict)
    children: list["CdiNode"] = field(default_factory=list)
    path: str = ""  # canonical tree path (joined by /), without index suffixes

    def children_named(self, name: str) -> list["CdiNode"]:
        return [c for c in self.children if c.name == name]


# ---------------------------------------------------------------------------
# Parsing
# ---------------------------------------------------------------------------


def _child_name(elem: ET.Element) -> str | None:
    name_elem = elem.find("name")
    if name_elem is None or name_elem.text is None:
        return None
    return name_elem.text.strip()


def _parse_enum(elem: ET.Element) -> dict[int, str]:
    out: dict[int, str] = {}
    map_elem = elem.find("map")
    if map_elem is None:
        return out
    for rel in map_elem.findall("relation"):
        prop = rel.find("property")
        val = rel.find("value")
        if prop is None or val is None or prop.text is None or val.text is None:
            continue
        try:
            n = int(prop.text.strip())
        except ValueError:
            continue
        out[n] = val.text.strip()
    return out


def _build_child(elem: ET.Element, parent_path: str) -> CdiNode | None:
    tag = elem.tag
    name = _child_name(elem)
    if name is None:
        return None
    path = f"{parent_path}/{name}" if parent_path else name

    if tag == "group":
        rep_elem = elem.find("repname")
        repname = rep_elem.text.strip() if rep_elem is not None and rep_elem.text else None
        try:
            replication = int(elem.get("replication", "1"))
        except ValueError:
            replication = 1
        node = CdiNode(
            name=name,
            kind="group",
            repname=repname,
            replication=replication,
            path=path,
        )
        for child in elem:
            sub = _build_child(child, path)
            if sub is not None:
                node.children.append(sub)
        return node

    if tag in LEAF_KINDS:
        return CdiNode(
            name=name,
            kind="leaf",
            enum_map=_parse_enum(elem),
            path=path,
        )

    if tag == EVENTID_KIND:
        return CdiNode(name=name, kind="eventid", path=path)

    return None


def _build_segment(elem: ET.Element) -> CdiNode | None:
    name = _child_name(elem)
    if name is None:
        return None
    node = CdiNode(name=name, kind="segment", path=name)
    for child in elem:
        sub = _build_child(child, name)
        if sub is not None:
            node.children.append(sub)
    return node


def parse_cdi(cdi_path: Path) -> CdiNode:
    """Parse a CDI XML file into a `CdiNode` tree rooted at a synthetic root."""
    tree = ET.parse(cdi_path)
    root_elem = tree.getroot()
    root = CdiNode(name="", kind="root", path="")
    for child in root_elem:
        if child.tag == "segment":
            node = _build_segment(child)
            if node is not None:
                root.children.append(node)
    return root


def parse_identification(cdi_path: Path) -> dict[str, str]:
    """Return `{manufacturer, model, hardwareVersion?, softwareVersion?}`."""
    tree = ET.parse(cdi_path)
    out: dict[str, str] = {}
    ident = tree.getroot().find("identification")
    if ident is None:
        return out
    for key in ("manufacturer", "model", "hardwareVersion", "softwareVersion"):
        el = ident.find(key)
        if el is not None and el.text is not None:
            out[key] = el.text.strip()
    return out


# ---------------------------------------------------------------------------
# Lookup
# ---------------------------------------------------------------------------


def _parse_suffix(chunk: str) -> tuple[str, tuple[int, int] | None]:
    m = INDEX_SUFFIX_RE.search(chunk)
    if not m:
        return chunk, None
    bare = chunk[: m.start()]
    lo = int(m.group(1))
    hi = int(m.group(2)) if m.group(2) is not None else lo
    return bare, (lo, hi)


def _pick_child(node: CdiNode, bare: str, suffix: tuple[int, int] | None) -> CdiNode | None:
    matches = node.children_named(bare)
    if not matches:
        return None
    if suffix is None:
        # Unambiguous when only one match; otherwise default to first.
        return matches[0]
    lo, hi = suffix
    if lo == hi:
        # Single-index disambiguator: 0-based ordinal among same-name siblings.
        if 0 <= lo < len(matches):
            return matches[lo]
        if 1 <= lo <= len(matches):
            return matches[lo - 1]
        return matches[-1]
    # Range suffix like [1-4]: prefer a sibling whose replication matches the span.
    span = hi - lo + 1
    for m in matches:
        if m.replication == span:
            return m
    return matches[-1]


def _walk(parts: list[str], node: CdiNode) -> CdiNode | None:
    if not parts:
        return node
    # Try every prefix length so child names containing a literal '/'
    # (e.g. "Commands/Consumers") still match correctly.
    for take in range(1, len(parts) + 1):
        chunk = "/".join(parts[:take])
        bare, suffix = _parse_suffix(chunk)
        if not bare:
            continue
        # repname collapse: skip a component matching the current group's
        # <repname>; the parent already represents the replica template.
        if node.repname is not None and bare == node.repname:
            res = _walk(parts[take:], node)
            if res is not None:
                return res
            continue
        child = _pick_child(node, bare, suffix)
        if child is None:
            continue
        res = _walk(parts[take:], child)
        if res is not None:
            return res
    return None


def lookup(path: str, root: CdiNode) -> CdiNode | None:
    """Resolve a `/`-separated path (possibly with index suffixes) to a node."""
    return _walk(path.split("/"), root)


def resolve_chain(path: str, root: CdiNode) -> list[tuple[CdiNode, CdiNode]] | None:
    """Resolve `path` and return the chain of `(parent, child)` pairs walked
    to reach the leaf. The first parent is `root` (synthetic). Returns
    `None` when the path does not resolve.
    """
    return _walk_chain(path.split("/"), root, [])


def _walk_chain(
    parts: list[str],
    node: CdiNode,
    acc: list[tuple[CdiNode, CdiNode]],
) -> list[tuple[CdiNode, CdiNode]] | None:
    if not parts:
        return acc
    for take in range(1, len(parts) + 1):
        chunk = "/".join(parts[:take])
        bare, suffix = _parse_suffix(chunk)
        if not bare:
            continue
        if node.repname is not None and bare == node.repname:
            res = _walk_chain(parts[take:], node, acc)
            if res is not None:
                return res
            continue
        child = _pick_child(node, bare, suffix)
        if child is None:
            continue
        res = _walk_chain(parts[take:], child, acc + [(node, child)])
        if res is not None:
            return res
    return None


# ---------------------------------------------------------------------------
# Walking / canonical path emission
# ---------------------------------------------------------------------------


def iter_all(node: CdiNode) -> Iterator[CdiNode]:
    """Yield every node in the tree (depth-first), including `node` itself."""
    yield node
    for child in node.children:
        yield from iter_all(child)


def walk_with_parents(root: CdiNode) -> Iterator[tuple[CdiNode, list[CdiNode]]]:
    """Yield `(node, ancestors_root_first)` for every node under `root`.

    The root itself is yielded first with an empty parents list.
    """

    def _w(current: CdiNode, parents: list[CdiNode]):
        yield current, list(parents)
        for child in current.children:
            yield from _w(child, parents + [current])

    yield from _w(root, [])


def canonical_emit_path(node: CdiNode, parents: list[CdiNode]) -> str:
    """Return the path a skill should emit for `node`, including the index
    suffix needed when a sibling collision (same `<name>`) requires
    disambiguation.

    Convention used by existing extraction files:
    - Unique name among siblings (any replication) → no suffix.
    - Two-or-more siblings sharing a name:
      - A replicated sibling with replication=N → `Name[1-N]`.
      - Any other (typically the unreplicated first one) → `Name[<ordinal>]`
        where ordinal is the 0-based position among same-name siblings.
    """
    # Drop the synthetic root from the chain when emitting paths.
    chain: list[CdiNode] = [p for p in parents if p.kind != "root"] + [node]
    pieces: list[str] = []
    for idx, n in enumerate(chain):
        parent = chain[idx - 1] if idx > 0 else None
        if parent is None:
            pieces.append(n.name)
            continue
        siblings = parent.children_named(n.name)
        if len(siblings) <= 1:
            pieces.append(n.name)
            continue
        if n.kind == "group" and n.replication > 1:
            pieces.append(f"{n.name}[1-{n.replication}]")
        else:
            pieces.append(f"{n.name}[{siblings.index(n)}]")
    return "/".join(pieces)
