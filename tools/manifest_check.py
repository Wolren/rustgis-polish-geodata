"""Checks plugin.json and the package against the rules the RustGIS plugin registry enforces.

The registry's `scripts/build.py` is the gate a published plugin passes, and RustGIS's
`crates/plugins/src/manifest.rs` is the loader. This script mirrors both, so a mistake fails here
instead of in a pull request.

    python tools/manifest_check.py                 the manifest beside this repository
    python tools/manifest_check.py <package.rgplugin>   a built package as well
    python tools/manifest_check.py --release       every platform binary must be in the package
"""

import json
import os
import re
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

ID_RE = re.compile(r"^[a-z][a-z0-9-]{0,63}$")
VERSION_RE = re.compile(r"^\d+(\.\d+){0,2}([-+][0-9A-Za-z]+(\.[0-9A-Za-z]+)*)*$")
PERMISSIONS = {"edit", "files", "network"}
PARAM_KINDS = {
    "layer", "vectorLayer", "polygonLayer", "pointLayer", "field", "numericField",
    "number", "text", "choice", "bool", "file",
}
EXTENSION_RE = re.compile(r"[A-Za-z0-9]{1,16}")
RUN_PLATFORMS = ("windows", "macos", "linux", "default")

# crates/plugins/src/manifest.rs
MAX_COMMANDS = 256
MAX_TOOLS = 64
MAX_PARAMS = 32
MAX_GROUPS = 8
MAX_GROUP_ITEMS = 16
MAX_MENU_ITEMS = 32
MAX_MENU_DEPTH = 4
MAX_TAB_LABEL = 32
MAX_RUN_ARGS = 64
MAX_SKILLS = 32
MAX_TEXT = 2_000
MAX_ENTRIES = 4096
MAX_UNPACKED = 1024 * 1024 * 1024

problems = []


def bad(message):
    problems.append(message)


def check_manifest(manifest, where):
    for key in ("id", "name", "version"):
        if not isinstance(manifest.get(key), str) or not manifest[key].strip():
            bad(f"{where}: plugin.json needs '{key}'")
    if not ID_RE.match(manifest.get("id", "")):
        bad(f"{where}: id must be lower-case letters, digits and hyphens")
    if not VERSION_RE.match(manifest.get("version", "")):
        bad(f"{where}: version must look like 1.0.0")
    if manifest.get("abi", 1) != 1:
        bad(f"{where}: abi must be 1")
    for permission in manifest.get("permissions", []):
        if permission not in PERMISSIONS:
            bad(f"{where}: unknown permission '{permission}'")
    homepage = manifest.get("homepage")
    if homepage and not str(homepage).startswith("https://"):
        bad(f"{where}: homepage must be an https:// address")
    if not manifest.get("commands") and not manifest.get("tools"):
        bad(f"{where}: the plugin adds no commands and no tools")
    runtime = manifest.get("runtime", "wasm")
    if runtime not in ("wasm", "process"):
        bad(f"{where}: runtime must be 'wasm' or 'process'")
    if runtime == "process" and "entry" in manifest:
        bad(f"{where}: a process plugin has no 'entry'")
    if len(manifest.get("commands", [])) > MAX_COMMANDS:
        bad(f"{where}: {len(manifest['commands'])} commands, the host allows {MAX_COMMANDS}")
    if len(manifest.get("tools", [])) > MAX_TOOLS:
        bad(f"{where}: {len(manifest['tools'])} tools, the host allows {MAX_TOOLS}")
    if len(manifest.get("skills", [])) > MAX_SKILLS:
        bad(f"{where}: {len(manifest['skills'])} skills, the host allows {MAX_SKILLS}")
    for key in ("name", "description"):
        if len(str(manifest.get(key, ""))) > MAX_TEXT:
            bad(f"{where}: {key} is longer than {MAX_TEXT} characters")
    if "\n" in str(manifest.get("description", "")):
        bad(f"{where}: the description must stay on one line")
    for command in manifest.get("commands", []):
        if not ID_RE.match(command.get("id", "")) or not command.get("label"):
            bad(f"{where}: command {command.get('id')!r} needs a valid id and a label")
        label = command.get("label", "")
        if "\n" in label:
            # The ribbon draws a label's own line break on purpose.
            pass
        if len(label) > MAX_TEXT:
            bad(f"{where}: command {command['id']}: label too long")
    for tool in manifest.get("tools", []):
        if not ID_RE.match(tool.get("id", "")) or not tool.get("name"):
            bad(f"{where}: tool {tool.get('id')!r} needs a valid id and a name")
        params = tool.get("params", [])
        if len(params) > MAX_PARAMS:
            bad(f"{where}: tool {tool['id']}: {len(params)} parameters, the host allows {MAX_PARAMS}")
        for param in params:
            if param.get("kind") not in PARAM_KINDS:
                bad(f"{where}: tool {tool['id']}: parameter {param.get('name')!r} has unknown kind")
            if param.get("kind") == "file" and "files" not in manifest.get("permissions", []):
                bad(f"{where}: tool {tool['id']}: a file parameter needs the 'files' permission")
            extensions = param.get("extensions", [])
            if not isinstance(extensions, list) or len(extensions) > 16:
                bad(f"{where}: tool {tool['id']}: parameter {param.get('name')!r} has invalid extensions")
            elif not all(isinstance(e, str) and EXTENSION_RE.fullmatch(e) for e in extensions):
                bad(f"{where}: tool {tool['id']}: parameter {param.get('name')!r} has invalid extensions")
    check_tab(manifest, where)
    check_panel(manifest, where)
    for skill in manifest.get("skills", []):
        if not ID_RE.match(skill):
            bad(f"{where}: skill name {skill!r} must be lower-case letters, digits and hyphens")


def check_tab(manifest, where):
    tab = manifest.get("tab")
    if tab is None:
        return
    label = str(tab.get("label", "")).strip()
    if not label or len(label) > MAX_TAB_LABEL or "\n" in label:
        bad(f"{where}: the tab label must be one line of 1 to {MAX_TAB_LABEL} characters")
    commands = {c.get("id") for c in manifest.get("commands", [])}
    tools = {t.get("id") for t in manifest.get("tools", [])}

    def entry(item):
        if isinstance(item, str):
            ok = (item[5:] in tools if item.startswith("tool:")
                  else item in commands or (item == "panel" and isinstance(manifest.get("panel"), dict)))
            if not ok:
                bad(f"{where}: the tab names {item!r}, which is not a command, a tool or the panel")

    groups = tab.get("groups") or []
    if not 1 <= len(groups) <= MAX_GROUPS:
        bad(f"{where}: the tab needs 1 to {MAX_GROUPS} groups, has {len(groups)}")
    for group in groups:
        items = group.get("items") or []
        if not group.get("label") or not 1 <= len(items) <= MAX_GROUP_ITEMS:
            bad(f"{where}: group {group.get('label')!r} needs a label and 1 to {MAX_GROUP_ITEMS} items")
        for item in items:
            if isinstance(item, dict):
                leaves = item.get("items") or []
                if not item.get("menu") or not 1 <= len(leaves) <= MAX_MENU_ITEMS:
                    bad(f"{where}: menu {item.get('menu')!r} needs a label and 1 to {MAX_MENU_ITEMS} items")
                for leaf in leaves:
                    entry(leaf)
            else:
                entry(item)


def check_panel(manifest, where):
    panel = manifest.get("panel")
    if panel is None:
        return
    commands = {c.get("id") for c in manifest.get("commands", [])}
    widgets = panel.get("items", [])
    kinds = {"text", "checkbox", "slider", "button", "tree"}
    for item in widgets:
        kind = item.get("kind")
        if kind not in kinds:
            bad(f"{where}: panel item {kind!r} has an unknown kind")
            continue
        if kind in ("button", "tree") and item.get("command") not in commands:
            bad(f"{where}: panel item {item.get('command')!r} is not one of the plugin's commands")
    for widget in widgets:
        if widget.get("kind") == "slider":
            low, high = widget.get("min"), widget.get("max")
            if not isinstance(low, (int, float)) or not isinstance(high, (int, float)) or high <= low:
                bad(f"{where}: panel slider needs min < max")


def check_package(path, manifest_path, release):
    manifest = json.load(open(manifest_path, encoding="utf-8"))
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        if len(infos) > MAX_ENTRIES:
            bad(f"package: {len(infos)} files, the limit is {MAX_ENTRIES}")
        if sum(i.file_size for i in infos) > MAX_UNPACKED:
            bad("package: unpacks to more than 1 GB")
        names = [i.filename for i in infos]
        root = "" if "plugin.json" in names else next(
            (n[: -len("plugin.json")] for n in names if n.endswith("/plugin.json")), "")
        files = {i.filename[len(root):]: i for i in infos if not i.is_dir()}
        if "plugin.json" not in files:
            bad("package: no plugin.json at its root")
        else:
            inside = json.loads(archive.read(root + "plugin.json"))
            if inside.get("id") != manifest.get("id") or inside.get("version") != manifest.get("version"):
                bad("package: plugin.json inside differs from the one beside it")
            run = inside.get("run")
            if isinstance(run, dict):
                for platform, argv in run.items():
                    if platform not in RUN_PLATFORMS:
                        bad(f"package: run names platform {platform!r}")
                        continue
                    program = argv[0].removeprefix("./")
                    if program not in files:
                        # A local build may hold one platform; a published package must hold them all.
                        if release:
                            bad(f"package: run.{platform} starts {program}, which is not in the package")
                        else:
                            print(f"manifest_check: no {platform} binary in this build, a release needs one")
                        continue
                    mode = files[program].external_attr >> 16
                    if platform != "windows" and not program.lower().endswith((".exe", ".bat", ".cmd")):
                        if not mode & 0o111:
                            bad(f"package: run.{platform} starts {program} without the execute bit")
            for skill in inside.get("skills", []):
                if f"skills/{skill}/SKILL.md" not in files:
                    bad(f"package: skill {skill!r} has no skills/{skill}/SKILL.md")


def main() -> int:
    manifest_path = os.path.join(ROOT, "plugin.json")
    manifest = json.load(open(manifest_path, encoding="utf-8"))
    check_manifest(manifest, "plugin.json")
    release = "--release" in sys.argv
    argv = [a for a in sys.argv[1:] if a != "--release"]
    package = argv[0] if argv else None
    if package is None:
        # The package this manifest builds, not whatever else sits in dist from an older build.
        expected = f"{manifest['id']}-{manifest['version']}.rgplugin"
        candidate = os.path.join(ROOT, "dist", expected)
        if os.path.exists(candidate):
            package = candidate
    if package:
        check_package(package, manifest_path, release)
        print(f"manifest_check: {os.path.basename(package)} checked")
    for problem in problems:
        print(problem)
    print(f"manifest_check: {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
