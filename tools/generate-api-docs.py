#!/usr/bin/env python3
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

"""Generate mdBook API reference from OpenAPI spec.

Usage:
    python3 tools/generate-api-docs.py

Reads:  crates/web-host/openapi.yaml
Writes: book/src/web-client/http-api-reference.md
"""

import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    sys.exit("pyyaml is required: pip install pyyaml")

ROOT = Path(__file__).resolve().parent.parent
SPEC_PATH = ROOT / "crates" / "web-host" / "openapi.yaml"
OUT_PATH = ROOT / "book" / "src" / "web-client" / "http-api-reference.md"

HTTP_METHODS = ["get", "post", "put", "delete", "patch", "head", "options", "trace"]


def load_spec():
    with open(SPEC_PATH) as f:
        return yaml.safe_load(f)


def resolve_ref(spec, ref):
    """Resolve a $ref like '#/components/schemas/Foo'."""
    parts = ref.lstrip("#/").split("/")
    node = spec
    for p in parts:
        node = node[p]
    return node


def resolve_param(spec, param):
    """Resolve a parameter that may be a $ref."""
    if "$ref" in param:
        return resolve_ref(spec, param["$ref"])
    return param


def resolve_schema(spec, schema):
    """Resolve a schema that may be a $ref."""
    if schema and "$ref" in schema:
        return resolve_ref(spec, schema["$ref"])
    return schema


def schema_type_str(spec, schema):
    """Return a human-readable type string for a schema."""
    if not schema:
        return ""
    schema = resolve_schema(spec, schema)
    if not schema:
        return ""
    t = schema.get("type", "")
    fmt = schema.get("format", "")
    enum = schema.get("enum")
    if enum:
        return " \\| ".join(f"`{v}`" for v in enum)
    if fmt:
        return f"{t} ({fmt})"
    return t


def collect_operations(spec):
    """Return list of (method, path, operation) grouped by tag."""
    tags_order = [t["name"] for t in spec.get("tags", [])]
    tag_descriptions = {t["name"]: t.get("description", "").strip() for t in spec.get("tags", [])}
    by_tag = {t: [] for t in tags_order}

    for path, path_item in spec.get("paths", {}).items():
        for method in HTTP_METHODS:
            if method not in path_item:
                continue
            op = path_item[method]
            op_tags = op.get("tags", ["Untagged"])
            for tag in op_tags:
                if tag not in by_tag:
                    by_tag[tag] = []
                by_tag[tag].append((method.upper(), path, op))

    return tags_order, tag_descriptions, by_tag


def render_parameters(spec, params):
    """Render a parameters table."""
    if not params:
        return ""
    resolved = [resolve_param(spec, p) for p in params]
    lines = [
        "",
        "**Parameters**",
        "",
        "| Name | In | Type | Required | Description |",
        "|------|----|------|----------|-------------|",
    ]
    for p in resolved:
        name = p.get("name", "")
        location = p.get("in", "")
        schema = p.get("schema", {})
        type_str = schema_type_str(spec, schema)
        required = "Yes" if p.get("required") else "No"
        desc = p.get("description", "").replace("\n", " ").strip()
        default = schema.get("default")
        if default is not None:
            # Render booleans as lowercase JSON style
            if isinstance(default, bool):
                default = "true" if default else "false"
            desc += f" (default: `{default}`)"
        lines.append(f"| `{name}` | {location} | {type_str} | {required} | {desc} |")
    lines.append("")
    return "\n".join(lines)


def render_request_body(spec, body):
    """Render request body information."""
    if not body:
        return ""
    required = body.get("required", False)
    if required:
        lines = ["", "**Request body** (required)", ""]
    else:
        lines = ["", "**Request body**", ""]

    content = body.get("content", {})
    for content_type, media in content.items():
        lines.append(f"- Content-Type: `{content_type}`")
        schema = media.get("schema", {})
        schema = resolve_schema(spec, schema)
        if schema:
            render_schema_fields(spec, schema, lines)
        example = media.get("example")
        if example:
            lines.append("")
            lines.append("  Example:")
            lines.append(f"  ```")
            lines.append(f"  {example.strip()}")
            lines.append(f"  ```")
    lines.append("")
    return "\n".join(lines)


def render_schema_fields(spec, schema, lines):
    """Render schema properties as a nested list."""
    if not schema:
        return

    # Handle oneOf
    one_of = schema.get("oneOf")
    if one_of:
        lines.append("")
        lines.append("  One of:")
        for variant in one_of:
            variant = resolve_schema(spec, variant)
            title = variant.get("title", "")
            # Try to get a distinguishing property
            props = variant.get("properties", {})
            type_prop = props.get("type", {})
            type_enum = type_prop.get("enum", [])
            if type_enum:
                lines.append(f"  - `type: \"{type_enum[0]}\"` — see schema below")
            elif title:
                lines.append(f"  - {title}")
        return

    props = schema.get("properties", {})
    required_fields = set(schema.get("required", []))
    if not props:
        return

    lines.append("")
    lines.append("  | Field | Type | Required | Description |")
    lines.append("  |-------|------|----------|-------------|")
    for name, prop in props.items():
        prop = resolve_schema(spec, prop)
        type_str = schema_type_str(spec, prop)
        req = "Yes" if name in required_fields else "No"
        desc = prop.get("description", "").replace("\n", " ").strip()
        lines.append(f"  | `{name}` | {type_str} | {req} | {desc} |")


def render_responses(spec, responses):
    """Render responses section."""
    if not responses:
        return ""
    lines = ["", "**Responses**", ""]
    for status, resp in sorted(responses.items()):
        if "$ref" in resp:
            resp = resolve_ref(spec, resp["$ref"])
        desc = resp.get("description", "").replace("\n", " ").strip()
        lines.append(f"- **{status}**: {desc}")

        # Show response headers if present
        headers = resp.get("headers", {})
        if headers:
            for hname, hinfo in headers.items():
                hdesc = hinfo.get("description", "").replace("\n", " ").strip()
                if hdesc:
                    lines.append(f"  - Header `{hname}`: {hdesc}")
                else:
                    lines.append(f"  - Header `{hname}`")

        # Show response content types
        content = resp.get("content", {})
        for ct in content:
            lines.append(f"  - Content-Type: `{ct}`")

    lines.append("")
    return "\n".join(lines)


def render_security(spec, security):
    """Render security requirements."""
    if not security:
        return ""
    schemes = []
    for req in security:
        for name in req:
            schemes.append(name)
    if not schemes:
        return ""
    labels = []
    for s in schemes:
        scheme_def = spec.get("components", {}).get("securitySchemes", {}).get(s, {})
        header_name = scheme_def.get("name", s)
        labels.append(f"`{header_name}`")
    return f"\nRequires: {', '.join(labels)}\n"


def generate():
    spec = load_spec()
    tags_order, tag_descriptions, by_tag = collect_operations(spec)

    lines = [
        "<!-- Generated from crates/web-host/openapi.yaml — do not edit by hand. -->",
        "<!-- Regenerate with: python3 tools/generate-api-docs.py -->",
        "",
        "# HTTP API Reference",
        "",
        spec.get("info", {}).get("description", "").strip(),
        "",
    ]

    for tag in tags_order:
        ops = by_tag.get(tag, [])
        if not ops:
            continue

        lines.append(f"## {tag}")
        lines.append("")
        tag_desc = tag_descriptions.get(tag, "")
        if tag_desc:
            lines.append(tag_desc)
            lines.append("")

        for method, path, op in ops:
            summary = op.get("summary", "")
            lines.append(f"### `{method} {path}`")
            lines.append("")
            if summary:
                lines.append(f"**{summary}**")
                lines.append("")
            description = op.get("description", "").strip()
            if description:
                lines.append(description)
                lines.append("")

            security = op.get("security")
            sec_text = render_security(spec, security)
            if sec_text:
                lines.append(sec_text)

            params = op.get("parameters")
            params_text = render_parameters(spec, params)
            if params_text:
                lines.append(params_text)

            body = op.get("requestBody")
            body_text = render_request_body(spec, body)
            if body_text:
                lines.append(body_text)

            resps = op.get("responses")
            resps_text = render_responses(spec, resps)
            if resps_text:
                lines.append(resps_text)

            lines.append("---")
            lines.append("")

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT_PATH, "w") as f:
        f.write("\n".join(lines))

    print(f"Wrote {OUT_PATH}")


if __name__ == "__main__":
    generate()
