#!/usr/bin/env python3
"""mdBook preprocessor: draws each `mermaid`, `plantuml`, `puml`, `dot`, or `graphviz` code block
with sirene and adds the drawing and the `sirene ascii` text below it, as on the landing page.

The sirene binary is $SIRENE, else target/release/sirene or target/debug/sirene in the
repository, else `sirene` on the PATH. A block sirene reports an error for fails the build; warnings
are printed and the block is still drawn. Put `no-render` after the language
(```mermaid no-render) to leave a block as source only.
"""

import base64
import html
import json
import os
import re
import shutil
import subprocess
import sys

LANGS = {
    "mermaid": "mermaid",
    "plantuml": "plantuml",
    "puml": "plantuml",
    "dot": "dot",
    "graphviz": "dot",
}

FENCE = re.compile(r"^( {0,3})(`{3,}|~{3,})(.*)$")


def sirene_bin():
    if os.environ.get("SIRENE"):
        return os.environ["SIRENE"]
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    for build in ("release", "debug"):
        path = os.path.join(root, "target", build, "sirene")
        if os.path.isfile(path):
            return path
    found = shutil.which("sirene")
    if found:
        return found
    sys.exit("samples: no sirene binary; build it (cargo build --release -p sirene "
             "--no-default-features) or set SIRENE")


def run(binary, args, source, where):
    proc = subprocess.run([binary, *args, "-"], input=source.encode(), capture_output=True)
    err = proc.stderr.decode().replace("<stdin>", where)
    if err:
        sys.stderr.write(err)
    if proc.returncode != 0:
        raise SystemExit(f"samples: sirene {args[0]} failed for {where}")
    return proc.stdout.decode()


def sample(binary, lang, source, where):
    svg = run(binary, ["svg", "--theme", "dark", "--lang", lang], source, where)
    ascii_text = run(binary, ["ascii", "--lang", lang], source, where).rstrip("\n")
    data = base64.b64encode(svg.encode()).decode()
    # One line, so Markdown keeps the whole block as raw HTML (a blank line would end it).
    text = html.escape(ascii_text, quote=False).replace("\n", "&#10;")
    return (
        '<div class="sirene-sample">'
        '<div class="sirene-sample-bar" role="tablist" aria-label="Output">'
        '<button class="sirene-view is-active" type="button" role="tab" aria-selected="true" '
        'data-view="drawing">drawing</button>'
        '<button class="sirene-view" type="button" role="tab" aria-selected="false" '
        'data-view="ascii">sirene ascii</button>'
        "</div>"
        '<div class="sirene-pane sirene-drawing" data-pane="drawing">'
        f'<img alt="the drawing of the {lang} source above" src="data:image/svg+xml;base64,{data}">'
        "</div>"
        f'<pre class="sirene-pane sirene-ascii" data-pane="ascii" hidden>{text}</pre>'
        "</div>"
    )


def process(binary, content, path):
    lines = content.split("\n")
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        out.append(line)
        m = FENCE.match(line)
        i += 1
        if not m:
            continue
        indent, fence, info = m.groups()
        words = info.split()
        start = i
        # The closing fence: same character, at least as long, nothing after it.
        close = re.compile(r"^ {0,3}" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}\s*$")
        while i < len(lines) and not close.match(lines[i]):
            out.append(lines[i])
            i += 1
        body = lines[start:i]
        if i < len(lines):
            out.append(lines[i])
            i += 1
        if not words or words[0] not in LANGS or "no-render" in words[1:]:
            continue
        source = "\n".join(l[len(indent):] if l.startswith(indent) else l.lstrip() for l in body)
        where = f"{path}:{start + 1}"
        out.append("")
        out.append(indent + sample(binary, LANGS[words[0]], source + "\n", where))
        out.append("")
    return "\n".join(out)


def walk(binary, items):
    for item in items:
        chapter = item.get("Chapter") if isinstance(item, dict) else None
        if not chapter:
            continue
        if chapter.get("path"):
            chapter["content"] = process(binary, chapter["content"], chapter["path"])
        walk(binary, chapter.get("sub_items", []))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "supports":
        sys.exit(0 if sys.argv[2] == "html" else 1)
    _context, book = json.load(sys.stdin)
    binary = sirene_bin()
    walk(binary, book.get("items", book.get("sections", [])))
    json.dump(book, sys.stdout)


if __name__ == "__main__":
    main()
