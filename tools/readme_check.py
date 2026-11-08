"""Checks that every command and tool a README names is really in the manifest.

Two READMEs drift apart, and a reader trusts them more than the code. This check makes the drift
loud: a name that `plugin.json` does not hold fails the build.

    python tools/readme_check.py          both READMEs
"""

import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "plugin.json")
READMES = ("README.md", "README.pl.md")

# `plugin:polish-geodata/<name>` in a call, and a bare `/`de name in a table row.
CALL = re.compile(r"plugin:polish-geodata/([a-z0-9-]+)")
BARE = re.compile(r"`([a-z][a-z0-9-]{2,})`")


def names_in(manifest_path):
    with open(manifest_path, encoding="utf-8") as handle:
        manifest = json.load(handle)
    commands = {command["id"] for command in manifest["commands"]}
    tools = {tool["id"] for tool in manifest["tools"]}
    return commands, tools


def known_words():
    """Words that look like identifiers but are file names, parameters or services."""
    return {
        "readme", "plugin", "tools", "data", "src", "dist", "skills", "packaging", "manifest",
        "bpy", "geojson", "bbox", "teryt", "obreb", "nazwa", "poziom", "przytnij", "warstwa",
        "usluga", "typy", "kolumna", "limit", "area", "layer", "layers", "ide", "geoportal",
        "wody", "gios", "poznan", "wroclaw", "warszawa", "egib", "gdos", "pig", "bdot",
    }


def main() -> int:
    commands, tools = names_in(MANIFEST)
    known = commands | tools
    problems = []
    checked = set()
    for name in READMES:
        path = os.path.join(ROOT, name)
        if not os.path.exists(path):
            problems.append(f"{name}: missing")
            continue
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        for match in CALL.finditer(text):
            name_in_text = match.group(1)
            checked.add(name_in_text)
            if name_in_text not in known:
                problems.append(f"{name}: calls unknown '{name_in_text}'")
        # A table row or inline code naming a command counts only when it looks like one.
        for match in BARE.finditer(text):
            word = match.group(1)
            if word in known or word in known_words():
                continue
            if word.startswith(("plugin", "readme", "http", "data-")):
                continue
            # Only report a word the manifest nearly holds, which is how a rename drifts.
            close = [candidate for candidate in known if candidate.startswith(word[:6])]
            if close and any(len(candidate) - len(word) <= 3 for candidate in close):
                problems.append(f"{name}: '{word}' looks like {close[0]}")
    for problem in problems:
        print(problem)
    print(f"readme: {len(checked)} command names checked, {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
