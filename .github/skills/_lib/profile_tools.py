# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml>=6.0"]
# ///
"""Profile-extraction tooling for LCC node profiles.

One CLI with subcommands shared across the `profile-*` skills:

    uv run .github/skills/_lib/profile_tools.py <subcommand> [args]

Subcommands:
    validate   <node-dir>                 (profile-6) cross-check extraction files
    assemble   <node-dir>                 (profile-7) build .profile.yaml
    skeleton   <kind> <node-dir>          (profile-1/3/4) emit blank scaffold
                                           kind ∈ {sections, fields, events}
    enum-fields <node-dir>                (profile-2) list enum fields and maps
    check      <node-dir> <cdiPath>       (any) ad-hoc path/value lookup
               [--value N]

Every subcommand expects a *node directory* (e.g.
`profile-extractions/signal-lcc`) that contains a `manual-outline.json`
with a `cdiFile` field pointing at the CDI XML. The CDI path is read
from that outline; nothing else needs to be passed in.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import yaml

# Local import: cdi_registry.py sits next to this script.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from cdi_registry import (  # noqa: E402
    CdiNode,
    canonical_emit_path,
    iter_all,
    lookup,
    parse_cdi,
    parse_identification,
    resolve_chain,
    walk_with_parents,
)


# ---------------------------------------------------------------------------
# Common helpers
# ---------------------------------------------------------------------------


def load_outline(node_dir: Path) -> dict[str, Any]:
    outline_path = node_dir / "manual-outline.json"
    if not outline_path.exists():
        raise SystemExit(f"manual-outline.json not found at {outline_path}")
    return json.loads(outline_path.read_text(encoding="utf-8"))


def load_cdi(node_dir: Path) -> tuple[CdiNode, Path, dict[str, Any]]:
    outline = load_outline(node_dir)
    cdi_path = Path(outline["cdiFile"])
    if not cdi_path.exists():
        raise SystemExit(f"CDI file not found at {cdi_path}")
    return parse_cdi(cdi_path), cdi_path, outline


def write_json(path: Path, data: Any) -> None:
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def write_yaml(path: Path, data: Any) -> None:
    path.write_text(
        yaml.safe_dump(data, sort_keys=False, allow_unicode=True, width=10_000),
        encoding="utf-8",
    )


def load_json_if_exists(path: Path) -> Any | None:
    if not path.exists():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def load_yaml_if_exists(path: Path) -> Any | None:
    if not path.exists():
        return None
    return yaml.safe_load(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# Subcommand: validate
# ---------------------------------------------------------------------------


def _record_path_error(
    errors: list[dict], rel_file: str, entry_id: str, path: str, msg: str
) -> None:
    errors.append({
        "extractionFile": rel_file,
        "entryId": entry_id,
        "referencedPath": path,
        "error": msg,
    })


def _record_enum_error(
    errors: list[dict],
    rel_file: str,
    entry_id: str,
    field_path: str,
    value: int,
    msg: str,
) -> None:
    errors.append({
        "extractionFile": rel_file,
        "entryId": entry_id,
        "field": field_path,
        "referencedValue": value,
        "error": msg,
    })


def _check_enum(
    root: CdiNode,
    errors: list[dict],
    rel_file: str,
    entry_id: str,
    field_path: str,
    value: int,
    label: str | None,
) -> None:
    node = lookup(field_path, root)
    if node is None or not node.enum_map:
        _record_enum_error(
            errors, rel_file, entry_id, field_path, value,
            "Field has no enum map (or path does not resolve).",
        )
        return
    if value not in node.enum_map:
        _record_enum_error(
            errors, rel_file, entry_id, field_path, value,
            f"Value {value} not in CDI map keys {sorted(node.enum_map.keys())}.",
        )
        return
    if label is not None and node.enum_map[value] != label:
        _record_enum_error(
            errors, rel_file, entry_id, field_path, value,
            f"Label mismatch: extraction says '{label}', CDI says '{node.enum_map[value]}'.",
        )


def cmd_validate(args: argparse.Namespace) -> int:
    node_dir = Path(args.node_dir).resolve()
    root, cdi_path, outline = load_cdi(node_dir)

    path_errors: list[dict] = []
    enum_errors: list[dict] = []
    files_used: list[str] = []

    covered_segments: set[str] = set()
    covered_groups: set[str] = set()
    covered_leaves: set[str] = set()
    covered_events: set[str] = set()

    # --- event-roles.json ---
    er = load_json_if_exists(node_dir / "event-roles.json")
    if er is not None:
        files_used.append("event-roles.json")
        for i, entry in enumerate(er.get("roles", [])):
            eid = f"roles[{i}] {entry.get('cdiPath', '<no path>')}"
            node = lookup(entry["cdiPath"], root)
            if node is None:
                _record_path_error(
                    path_errors, "event-roles.json", eid, entry["cdiPath"],
                    "Path does not resolve in CDI registry.",
                )
                continue
            for cf in entry.get("childFields", []):
                child_path = f"{entry['cdiPath']}/{cf}"
                sub = lookup(child_path, root)
                if sub is None:
                    _record_path_error(
                        path_errors, "event-roles.json", eid, child_path,
                        f"childField '{cf}' not found under '{entry['cdiPath']}'.",
                    )
                elif sub.kind == "eventid":
                    covered_events.add(sub.path)

    # --- relevance-rules.json ---
    rr = load_json_if_exists(node_dir / "relevance-rules.json")
    if rr is not None:
        files_used.append("relevance-rules.json")
        for entry in rr.get("rules", []):
            eid = entry.get("id", "<no id>")
            if lookup(entry["affectedSection"], root) is None:
                _record_path_error(
                    path_errors, "relevance-rules.json", eid,
                    entry["affectedSection"],
                    "Path does not resolve in CDI registry.",
                )
            ctrl = lookup(entry["controllingField"], root)
            if ctrl is None:
                _record_path_error(
                    path_errors, "relevance-rules.json", eid,
                    entry["controllingField"],
                    "Path does not resolve in CDI registry.",
                )
            else:
                for val in entry.get("irrelevantWhen", []):
                    _check_enum(
                        root, enum_errors, "relevance-rules.json", eid,
                        entry["controllingField"], int(val), None,
                    )

    # --- section-descriptions.yaml ---
    sd = load_yaml_if_exists(node_dir / "section-descriptions.yaml")
    if sd is not None:
        files_used.append("section-descriptions.yaml")
        for entry in sd.get("sections", []):
            eid = entry.get("cdiPath", "<no path>")
            node = lookup(entry["cdiPath"], root)
            if node is None:
                _record_path_error(
                    path_errors, "section-descriptions.yaml", eid,
                    entry["cdiPath"],
                    "Path does not resolve in CDI registry.",
                )
                continue
            level = entry.get("level")
            if level == "segment" and node.kind == "segment":
                covered_segments.add(node.path)
            elif level == "group" and node.kind == "group":
                covered_groups.add(node.path)

    # --- field-descriptions.yaml ---
    fd = load_yaml_if_exists(node_dir / "field-descriptions.yaml")
    if fd is not None:
        files_used.append("field-descriptions.yaml")
        for entry in fd.get("fields", []):
            eid = entry.get("cdiPath", "<no path>")
            node = lookup(entry["cdiPath"], root)
            if node is None:
                _record_path_error(
                    path_errors, "field-descriptions.yaml", eid,
                    entry["cdiPath"],
                    "Path does not resolve in CDI registry.",
                )
                continue
            if node.kind == "leaf":
                covered_leaves.add(node.path)
            elif node.kind == "eventid":
                covered_events.add(node.path)
            for opt in entry.get("options", []) or []:
                if "value" in opt:
                    _check_enum(
                        root, enum_errors, "field-descriptions.yaml", eid,
                        entry["cdiPath"], int(opt["value"]), opt.get("label"),
                    )

    # --- recipes.yaml ---
    rc = load_yaml_if_exists(node_dir / "recipes.yaml")
    if rc is not None:
        files_used.append("recipes.yaml")
        for ri, recipe in enumerate(rc.get("recipes", [])):
            rname = recipe.get("name", f"recipe[{ri}]")
            if "scope" in recipe and lookup(recipe["scope"], root) is None:
                _record_path_error(
                    path_errors, "recipes.yaml", rname, recipe["scope"],
                    "Path does not resolve in CDI registry.",
                )
            for si, step in enumerate(recipe.get("steps", []) or []):
                eid = f"{rname} step[{si + 1}]"
                field_path = step.get("field")
                if not field_path:
                    continue
                node = lookup(field_path, root)
                if node is None:
                    _record_path_error(
                        path_errors, "recipes.yaml", eid, field_path,
                        "Path does not resolve in CDI registry.",
                    )
                    continue
                raw = step.get("rawValue")
                if raw is None or node.kind != "leaf" or not node.enum_map:
                    continue
                try:
                    raw_int = int(raw)
                except (TypeError, ValueError):
                    continue
                value_label = step.get("value")
                _check_enum(
                    root, enum_errors, "recipes.yaml", eid, field_path,
                    raw_int,
                    value_label if isinstance(value_label, str) else None,
                )

    # --- coverage ---
    all_nodes = [n for n in iter_all(root) if n.kind != "root"]
    seg_paths = {n.path for n in all_nodes if n.kind == "segment"}
    grp_paths = {n.path for n in all_nodes if n.kind == "group"}
    leaf_paths = {n.path for n in all_nodes if n.kind == "leaf"}
    event_paths = {n.path for n in all_nodes if n.kind == "eventid"}

    covered_seg = covered_segments & seg_paths
    covered_grp = covered_groups & grp_paths
    covered_leaf = covered_leaves & leaf_paths
    covered_event = covered_events & event_paths

    total = len(seg_paths) + len(grp_paths) + len(leaf_paths) + len(event_paths)
    covered_total = (
        len(covered_seg) + len(covered_grp) + len(covered_leaf) + len(covered_event)
    )
    pct = round(100.0 * covered_total / total, 1) if total else 0.0

    uncovered: list[dict] = []
    for n in all_nodes:
        if n.kind == "segment" and n.path not in covered_seg:
            uncovered.append({"cdiPath": n.path, "level": "segment", "name": n.name})
        elif n.kind == "group" and n.path not in covered_grp:
            uncovered.append({"cdiPath": n.path, "level": "group", "name": n.name})
        elif n.kind == "leaf" and n.path not in covered_leaf:
            uncovered.append({"cdiPath": n.path, "level": "field", "name": n.name})
        elif n.kind == "eventid" and n.path not in covered_event:
            uncovered.append({"cdiPath": n.path, "level": "field", "name": n.name})

    passed = not path_errors and not enum_errors
    summary = (
        f"{'PASS' if passed else 'FAIL'}: {len(path_errors)} path errors, "
        f"{len(enum_errors)} enum errors, {pct}% coverage"
    )

    ident = parse_identification(cdi_path)
    report = {
        "nodeType": outline.get("nodeType")
        or {"manufacturer": ident.get("manufacturer", ""), "model": ident.get("model", "")},
        "cdiFile": str(cdi_path),
        "extractionFiles": files_used,
        "pathErrors": path_errors,
        "enumErrors": enum_errors,
        "coverage": {
            "totalSegments": len(seg_paths),
            "coveredSegments": len(covered_seg),
            "totalGroups": len(grp_paths),
            "coveredGroups": len(covered_grp),
            "totalLeafFields": len(leaf_paths),
            "coveredLeafFields": len(covered_leaf),
            "totalEventSlots": len(event_paths),
            "coveredEventSlots": len(covered_event),
            "overallPercentage": pct,
        },
        "uncoveredSections": uncovered,
        "summary": summary,
    }

    out_path = node_dir / "validation-report.json"
    write_json(out_path, report)
    print(summary)
    print(f"Report: {out_path}")
    if path_errors:
        print(f"Path errors ({len(path_errors)}):")
        for e in path_errors[:20]:
            print(" ", e)
    if enum_errors:
        print(f"Enum errors ({len(enum_errors)}):")
        for e in enum_errors[:20]:
            print(" ", e)
    if uncovered:
        print(f"Uncovered: {len(uncovered)} of {total}")
    return 0 if passed else 1


# ---------------------------------------------------------------------------
# Subcommand: assemble
# ---------------------------------------------------------------------------


def _convert_path(raw: str, root: CdiNode) -> tuple[str, str]:
    """Convert an extraction-format path to v2 profile notation, returning
    `(canonical_path, top_segment_name)`.

    For every component:
    - If the bare name has only one sibling at that level → emit just the name.
    - If multiple siblings share the name → emit `Name#<ordinal>` (1-based
      document order) regardless of whether the extraction used a `[N]` /
      `[N-M]` suffix or no suffix at all. This is the auto-disambiguation
      step that closes the v1 ambiguity gap.
    """
    chain = resolve_chain(raw, root)
    if chain is None:
        raise ValueError(f"Could not resolve path during assemble: {raw!r}")
    out_parts: list[str] = []
    for parent, child in chain:
        siblings = parent.children_named(child.name)
        if len(siblings) <= 1:
            out_parts.append(child.name)
        else:
            ordinal = siblings.index(child) + 1
            out_parts.append(f"{child.name}#{ordinal}")
    top_segment = chain[0][1].name if chain else ""
    return "/".join(out_parts), top_segment


def _yaml_dq(s: str) -> str:
    """YAML double-quoted scalar. JSON encoding is a strict subset of YAML."""
    return json.dumps(s, ensure_ascii=False)


def _segment_of(entry: dict) -> str:
    return entry.get("_segment", "")


def _render_explanation(text: str, indent: str) -> str:
    """Render a single-string field. Inline-quoted when no newline; literal
    block scalar (`|`) when the source contains newlines.
    """
    if "\n" not in text:
        return _yaml_dq(text)
    lines = text.splitlines()
    out = "|\n"
    for line in lines:
        out += f"{indent}  {line}\n".rstrip() + "\n"
    return out.rstrip()


_HEADER = (
    '# Structure profile for {mfr} / {model}\n'
    '#\n'
    '# Generated by .github/skills/_lib/profile_tools.py assemble from\n'
    '# the extraction outputs in this directory. Re-running assemble will\n'
    '# overwrite this file; hand edits should be re-applied or migrated\n'
    '# back into the extraction sources (event-roles.json,\n'
    '# relevance-rules.json) so they survive re-assembly.\n'
    '#\n'
    '# Schema: https://bowties.app/schemas/structure-profile-v2.schema.json\n'
    '# (see specs/014-config-modes-placeholders for the v1\u2192v2 changes).\n'
    '# The Bowties profile loader rejects schemaVersion "1.0".\n'
    '\n'
    'schemaVersion: "2.0"\n'
    '\n'
    '# Node type \u2014 matched against the SNIP manufacturer/model fields\n'
    '# reported by the node when Bowties connects.\n'
    'nodeType:\n'
    '  manufacturer: {mfr_q}\n'
    '  model: {model_q}\n'
    '\n'
)

_EVENT_ROLES_HEADER = (
    '# Event role declarations\n'
    '#\n'
    '# Each entry maps a name-based CDI group path to a declared role. All\n'
    '# eventid leaves inside the matching group \u2014 across every replicated\n'
    '# instance \u2014 receive this role, overriding any heuristic assignment.\n'
    '#\n'
    "# Path notation: '/'-separated CDI element names. Use '#N' (1-based)\n"
    '# to disambiguate same-named sibling groups (e.g. Conditionals/Logic/Action#2).\n'
    '# Paths that cannot be resolved in the connected node\'s CDI are\n'
    '# silently skipped with a log warning.\n'
    '\n'
    'eventRoles:\n'
)

_RELEVANCE_HEADER = (
    '# Relevance rules\n'
    '#\n'
    '# Each rule declares that a CDI section becomes irrelevant when the\n'
    '# listed controlling field has one of the given values. The\n'
    "# 'explanation' is shown VERBATIM in the UI banner \u2014 do not paraphrase\n"
    '# or substitute when editing.\n'
    '\n'
    'relevanceRules:\n'
)


def _render_event_roles(roles: list[dict]) -> str:
    out: list[str] = []
    last_segment: str | None = None
    for entry in roles:
        seg = _segment_of(entry)
        if seg != last_segment:
            if last_segment is not None:
                out.append("")
            out.append(f"  # \u2500\u2500 {seg} \u2500\u2500")
            last_segment = seg
        out.append(f"  - groupPath: {_yaml_dq(entry['groupPath'])}")
        out.append(f"    role: {entry['role']}")
        if entry.get("label"):
            out.append(f"    label: {_yaml_dq(entry['label'])}")
    return "\n".join(out) + "\n"


def _render_relevance_rules(rules: list[dict]) -> str:
    if not rules:
        return ""
    out: list[str] = []
    last_segment: str | None = None
    for entry in rules:
        seg = _segment_of(entry)
        if seg != last_segment:
            if last_segment is not None:
                out.append("")
            out.append(f"  # \u2500\u2500 {seg} \u2500\u2500")
            last_segment = seg
        out.append(f"  - id: {_yaml_dq(entry['id'])}")
        out.append(f"    affectedTarget: {_yaml_dq(entry['affectedTarget'])}")
        out.append("    allOf:")
        for cond in entry["allOf"]:
            values = ", ".join(str(v) for v in cond["irrelevantWhen"])
            out.append(f"      - field: {_yaml_dq(cond['field'])}")
            out.append(f"        irrelevantWhen: [{values}]")
        explanation = entry["explanation"]
        rendered = _render_explanation(explanation, indent="    ")
        out.append(f"    explanation: {rendered}")
    return "\n".join(out) + "\n"


def cmd_assemble(args: argparse.Namespace) -> int:
    node_dir = Path(args.node_dir).resolve()
    root, cdi_path, outline = load_cdi(node_dir)
    ident = outline.get("nodeType") or parse_identification(cdi_path)

    er_path = node_dir / "event-roles.json"
    rr_path = node_dir / "relevance-rules.json"
    if not er_path.exists():
        raise SystemExit(f"event-roles.json missing in {node_dir}")
    if not rr_path.exists():
        raise SystemExit(f"relevance-rules.json missing in {node_dir}")
    er = json.loads(er_path.read_text(encoding="utf-8"))
    rr = json.loads(rr_path.read_text(encoding="utf-8"))

    event_roles_out: list[dict] = []
    for entry in er.get("roles", []):
        gp, seg = _convert_path(entry["cdiPath"], root)
        event_roles_out.append({
            "groupPath": gp,
            "role": entry["role"],
            "_segment": seg,
        })

    rules_out: list[dict] = []
    for entry in rr.get("rules", []):
        ctrl_path, _ = _convert_path(entry["controllingField"], root)
        # `field` is the controlling field's display name, not a full path.
        field_name = ctrl_path.rsplit("/", 1)[-1]
        affected_path, affected_seg = _convert_path(entry["affectedSection"], root)
        rules_out.append({
            "id": entry["id"],
            "affectedTarget": affected_path,
            "allOf": [{
                "field": field_name,
                "irrelevantWhen": list(entry["irrelevantWhen"]),
            }],
            "explanation": entry["explanation"],
            "_segment": affected_seg,
        })

    manufacturer = ident.get("manufacturer", "")
    model = ident.get("model", "")

    out = _HEADER.format(
        mfr=manufacturer,
        model=model,
        mfr_q=_yaml_dq(manufacturer),
        model_q=_yaml_dq(model),
    )
    out += _EVENT_ROLES_HEADER
    if event_roles_out:
        out += _render_event_roles(event_roles_out)
    else:
        out = out.rstrip("\n") + " []\n"
    out += "\n"
    out += _RELEVANCE_HEADER
    if rules_out:
        out += _render_relevance_rules(rules_out)
    else:
        out = out.rstrip("\n") + " []\n"

    safe_mfr = manufacturer.replace(" ", "_").replace(",", "")
    safe_model = model.replace(" ", "_")
    filename = f"{safe_mfr}_{safe_model}.profile.yaml"
    out_path = node_dir / filename
    out_path.write_text(out, encoding="utf-8")
    print(f"Wrote {out_path}")
    print(f"  eventRoles: {len(event_roles_out)}")
    print(f"  relevanceRules: {len(rules_out)}")
    return 0


# ---------------------------------------------------------------------------
# Subcommand: skeleton
# ---------------------------------------------------------------------------


def _node_type(outline: dict[str, Any], cdi_path: Path) -> dict[str, str]:
    nt = outline.get("nodeType")
    if nt:
        return {"manufacturer": nt.get("manufacturer", ""), "model": nt.get("model", "")}
    ident = parse_identification(cdi_path)
    return {"manufacturer": ident.get("manufacturer", ""), "model": ident.get("model", "")}


def _skeleton_sections(root: CdiNode, node_type: dict[str, str]) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for node, parents in walk_with_parents(root):
        if node.kind == "segment":
            level = "segment"
        elif node.kind == "group":
            level = "group"
        else:
            continue
        entries.append({
            "cdiPath": canonical_emit_path(node, parents),
            "level": level,
            "name": node.name,
            "description": "TODO: 1-3 sentence purpose statement (Markdown allowed).",
            "citation": "TODO: manual section / page reference.",
        })
    return {"nodeType": node_type, "sections": entries}


def _skeleton_fields(root: CdiNode, node_type: dict[str, str]) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for node, parents in walk_with_parents(root):
        if node.kind not in ("leaf", "eventid"):
            continue
        entry: dict[str, Any] = {
            "cdiPath": canonical_emit_path(node, parents),
            "name": node.name,
            "elementType": "eventid" if node.kind == "eventid" else "int",  # refine below
            "description": "TODO: field description (Markdown allowed).",
        }
        if node.kind == "leaf":
            # We don't know int vs string from kind alone — surface raw tag
            # if useful by re-reading the source XML element. For skeleton
            # purposes assume 'int' when there's an enum_map, else 'string'.
            entry["elementType"] = "int" if node.enum_map else "string"
            entry["units"] = None
            entry["validRange"] = None
            entry["typicalValues"] = None
            if entry["elementType"] == "string":
                entry["maxLength"] = None
            if node.enum_map:
                entry["options"] = [
                    {
                        "value": v,
                        "label": label,
                        "description": "TODO",
                        "category": None,
                    }
                    for v, label in sorted(node.enum_map.items())
                ]
        else:
            entry["role"] = "TODO (Producer | Consumer)"
        entry["citation"] = "TODO: manual section / page reference."
        entries.append(entry)
    return {"nodeType": node_type, "fields": entries}


def _skeleton_events(root: CdiNode, node_type: dict[str, str]) -> dict[str, Any]:
    """Emit an event-roles skeleton: one entry per group that has at least
    one direct `<eventid>` child. The LLM fills role/citation/confidence.
    """
    entries: list[dict[str, Any]] = []
    for node, parents in walk_with_parents(root):
        if node.kind not in ("group", "segment"):
            continue
        event_children = [c for c in node.children if c.kind == "eventid"]
        if not event_children:
            continue
        entries.append({
            "cdiPath": canonical_emit_path(node, parents),
            "role": "TODO (Producer | Consumer)",
            "childFields": [c.name for c in event_children],
            "citation": "TODO: manual quote/reference.",
            "confidence": "TODO (High | Medium)",
            "notes": None,
        })
    return {"nodeType": node_type, "roles": entries}


SKELETON_KINDS = {
    "sections": (_skeleton_sections, "section-descriptions.skeleton.yaml", write_yaml),
    "fields": (_skeleton_fields, "field-descriptions.skeleton.yaml", write_yaml),
    "events": (_skeleton_events, "event-roles.skeleton.json", write_json),
}


def cmd_skeleton(args: argparse.Namespace) -> int:
    if args.kind not in SKELETON_KINDS:
        raise SystemExit(f"Unknown skeleton kind: {args.kind}. Use one of {sorted(SKELETON_KINDS)}.")
    node_dir = Path(args.node_dir).resolve()
    root, cdi_path, outline = load_cdi(node_dir)
    builder, default_name, writer = SKELETON_KINDS[args.kind]
    data = builder(root, _node_type(outline, cdi_path))
    out_path = node_dir / default_name
    writer(out_path, data)
    count_key = "fields" if args.kind == "fields" else "sections" if args.kind == "sections" else "roles"
    print(f"Wrote {out_path}")
    print(f"  entries: {len(data[count_key])}")
    return 0


# ---------------------------------------------------------------------------
# Subcommand: enum-fields
# ---------------------------------------------------------------------------


def cmd_enum_fields(args: argparse.Namespace) -> int:
    node_dir = Path(args.node_dir).resolve()
    root, _, _ = load_cdi(node_dir)
    rows: list[dict[str, Any]] = []
    for node, parents in walk_with_parents(root):
        if node.kind != "leaf" or not node.enum_map:
            continue
        rows.append({
            "cdiPath": canonical_emit_path(node, parents),
            "name": node.name,
            "values": [
                {"value": v, "label": label}
                for v, label in sorted(node.enum_map.items())
            ],
        })
    out = json.dumps({"enumFields": rows}, indent=2)
    print(out)
    print(f"\n# {len(rows)} enum fields in CDI", file=sys.stderr)
    return 0


# ---------------------------------------------------------------------------
# Subcommand: check
# ---------------------------------------------------------------------------


def cmd_check(args: argparse.Namespace) -> int:
    node_dir = Path(args.node_dir).resolve()
    root, _, _ = load_cdi(node_dir)
    node = lookup(args.path, root)
    if node is None:
        print(f"NOT FOUND: {args.path}")
        return 1
    info = {
        "path": node.path,
        "kind": node.kind,
        "replication": node.replication,
        "repname": node.repname,
        "children": [c.name for c in node.children],
        "enumMap": dict(sorted(node.enum_map.items())) if node.enum_map else None,
    }
    print(json.dumps(info, indent=2))
    if args.value is not None:
        if not node.enum_map:
            print(f"\nField has no enum map.", file=sys.stderr)
            return 1
        if args.value in node.enum_map:
            print(f"\nValue {args.value} -> '{node.enum_map[args.value]}' OK")
            return 0
        print(
            f"\nValue {args.value} NOT in map keys {sorted(node.enum_map)}",
            file=sys.stderr,
        )
        return 1
    return 0


# ---------------------------------------------------------------------------
# argparse wiring
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="profile_tools", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)

    v = sub.add_parser("validate", help="profile-6: cross-check extraction files")
    v.add_argument("node_dir")
    v.set_defaults(func=cmd_validate)

    a = sub.add_parser("assemble", help="profile-7: build .profile.yaml")
    a.add_argument("node_dir")
    a.set_defaults(func=cmd_assemble)

    s = sub.add_parser("skeleton", help="emit a blank scaffold for one extraction file")
    s.add_argument("kind", choices=sorted(SKELETON_KINDS))
    s.add_argument("node_dir")
    s.set_defaults(func=cmd_skeleton)

    e = sub.add_parser("enum-fields", help="list every enum field with its CDI map")
    e.add_argument("node_dir")
    e.set_defaults(func=cmd_enum_fields)

    c = sub.add_parser("check", help="resolve a path (optionally check an enum value)")
    c.add_argument("node_dir")
    c.add_argument("path")
    c.add_argument("--value", type=int, default=None)
    c.set_defaults(func=cmd_check)

    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
