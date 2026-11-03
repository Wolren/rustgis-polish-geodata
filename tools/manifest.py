"""Build plugin.json from the catalogues and the editorial manifest.

`data/services.json` holds the WMS services, `data/wfs.json` the WFS types. Both are built and
verified by `tools/services.py` and `tools/wfs.py`. Everything that is not a catalogue entry lives
in `tools/manifest.base.json`: the commands of the built-in layers, the download tools, the tab
layout, the panel and the translations.

Keeping the three apart is the point. A hand-written plugin.json drifted from the catalogues: 162
of the 270 WFS types had no command, every command carried a `command` key the host ignores, and
two datasets appeared under one title. Here a dataset that is missing from a group fails the build,
and a group that names a dataset the catalogue does not have fails it too.

Run: python tools/manifest.py           writes plugin.json
     python tools/manifest.py --check   exits 1 when plugin.json is out of date
"""

import io
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.path.join(ROOT, "tools", "manifest.base.json")
SERVICES = os.path.join(ROOT, "data", "services.json")
WFS = os.path.join(ROOT, "data", "wfs.json")
OUT = os.path.join(ROOT, "plugin.json")

# The host's limits (docs/plugins.md, "plugin.json").
MAX_COMMANDS = 256
MAX_TOOLS = 64
MAX_TOOL_PARAMS = 32
MAX_GROUPS = 8
MAX_GROUP_ITEMS = {"large": 16, "small": 48}
MAX_MENU_ITEMS = 32
MAX_MENU_DEPTH = 4
MAX_CHOICES = 64


class Bad(Exception):
    pass


def load(path):
    with io.open(path, encoding="utf-8") as handle:
        return json.load(handle)


def catalogue():
    """The WFS catalogue by id, and the ids the download tools may offer."""
    entries = load(WFS)
    by_id, offered = {}, []
    for entry in entries:
        for key in ("id", "label", "url", "type", "geom", "max_span"):
            if key not in entry:
                raise Bad(f"data/wfs.json: {entry.get('id', '?')} has no '{key}'")
        if entry["id"] in by_id:
            raise Bad(f"data/wfs.json: two entries share the id {entry['id']}")
        by_id[entry["id"]] = entry
        if not entry.get("index", False):
            offered.append(entry["id"])
    return by_id, offered


def menu_decl(key, menus, folders, depth=1):
    """A menu of the tab: a leaf of services, or a folder of leaves one level down."""
    if key in folders:
        if depth >= MAX_MENU_DEPTH:
            raise Bad(f"menu '{key}' nests deeper than the host's {MAX_MENU_DEPTH} levels")
        meta = folders[key]
        if not 1 <= len(meta["children"]) <= MAX_MENU_ITEMS:
            raise Bad(f"menu {meta['label']}: {len(meta['children'])} children")
        return {
            "menu": meta["label"],
            "icon": meta.get("icon", ""),
            "help": meta.get("help", ""),
            "items": [menu_decl(child, menus, folders, depth + 1) for child in meta["children"]],
        }
    meta = menus.get(key)
    if meta is None:
        raise Bad(f"the tab names menu '{key}', which no service and no folder defines")
    return {
        "menu": meta["label"],
        "icon": meta.get("icon", ""),
        "help": meta.get("help", ""),
        "items": meta["items"],
    }


def check_item(where, item, known):
    """Every name under a group is one of the plugin's commands or tools, at any menu depth."""
    if isinstance(item, dict):
        entries = item.get("items", [])
        if not 1 <= len(entries) <= MAX_MENU_ITEMS:
            raise Bad(f"menu {item.get('menu')}: {len(entries)} entries")
        for entry in entries:
            check_item(item.get("menu"), entry, known)
    elif item not in known:
        raise Bad(f"{where}: '{item}' is not a command or a tool")


def leaf_entries(groups):
    """Every command and tool a tab puts on the ribbon, menus included."""
    listed = []

    def walk(items):
        for item in items:
            if isinstance(item, dict):
                walk(item.get("items", []))
            else:
                listed.append(item)

    for group in groups:
        walk(group["items"])
    return listed


def service_help(service):
    """The tooltip of a service button: its label and the publisher, one string for both languages."""
    label = service.get("label") or service["name"]
    # A credit may name the portal too; the tooltip shows the institution alone.
    publisher = (service.get("source") or "").split(" \u00b7 ")[0] or "GUGiK"
    return f"{label} \u00b7 {publisher}"


def build():
    base = load(BASE)
    services = load(SERVICES)
    by_id, offered = catalogue()

    commands = list(base["commands"])
    tools = list(base["tools"])
    group_of_tool = {}

    # A tool's choices are the datasets whose catalogue entry names it, in the catalogue's order.
    for tool in tools:
        here = [entry for entry in by_id.values() if entry.get("tool") == tool["id"]]
        param = next((p for p in tool.get("params", []) if p["name"] == "dane"), None)
        if param is None:
            continue
        labels = []
        for entry in here:
            if entry["id"] not in offered:
                raise Bad(f"{entry['id']} is a sheet index, not offered")
            labels.append(entry["label"])
        if not labels:
            raise Bad(f"tool {tool['id']} has no dataset in the catalogue")
        if len(labels) > MAX_CHOICES:
            raise Bad(
                f"tool {tool['id']}: {len(labels)} choices, the host allows {MAX_CHOICES}"
            )
        if len(set(labels)) != len(labels):
            raise Bad(f"tool {tool['id']}: two datasets share a title")
        param["choices"] = labels
        group_of_tool[tool["id"]] = [entry["id"] for entry in here]

    # Every offered dataset belongs to exactly one tool, or the pickers cannot reach it.
    placed = {}
    for tool_id, ids in group_of_tool.items():
        for dataset in ids:
            if dataset in placed:
                raise Bad(f"'{dataset}' is in {placed[dataset]} and in {tool_id}")
            placed[dataset] = tool_id
    missing = [dataset for dataset in offered if dataset not in placed]
    if missing:
        raise Bad(f"offered but in no download tool: {', '.join(missing[:5])}")
    stray = [key for key in by_id if by_id[key].get("tool") not in group_of_tool and by_id[key].get("tool")]
    if stray:
        raise Bad(f"dataset names a tool that does not exist: {by_id[stray[0]]['tool']}")

    # One command per WMS service, from the catalogue rather than by hand.
    service_commands = []
    for service in services:
        cmd = service.get("cmd", "")
        if not cmd:
            raise Bad(f"data/services.json: {service.get('id', '?')} has no 'cmd'")
        service_commands.append(
            {
                "id": cmd,
                "label": service.get("label") or service["name"],
                "icon": "basemap",
                "help": service_help(service),
                "params": json.dumps({"service": service["name"]}, ensure_ascii=False),
            }
        )
    commands.extend(sorted(service_commands, key=lambda c: c["id"]))

    ids = [c["id"] for c in commands]
    for command_id in ids:
        if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", command_id):
            raise Bad(
                f"command id '{command_id}' is not lower-case letters, digits and hyphens "
                "(the host refuses it)"
            )
    if len(ids) != len(set(ids)):
        duplicates = sorted({i for i in ids if ids.count(i) > 1})
        raise Bad(f"two commands share an id: {duplicates[:5]}")
    if len(commands) > MAX_COMMANDS:
        raise Bad(f"{len(commands)} commands, the host allows {MAX_COMMANDS}")

    # The institution lives on each catalogue entry, so no list here can drift from the catalogue.
    by_institution = {}
    for service in services:
        key = service.get("institutionMenu") or service.get("institution")
        if not key:
            raise Bad(f"data/services.json: {service['id']} names no institution")
        by_institution.setdefault(key, []).append(service["cmd"])
    menus = {}
    for key, cmds in by_institution.items():
        meta = base["institutionMenus"].get(key)
        if meta is None:
            raise Bad(f"menu '{key}' has no label in tools/manifest.base.json")
        if "children" in meta:
            raise Bad(f"menu '{key}' holds themes, so it cannot hold services too")
        if not 1 <= len(cmds) <= MAX_MENU_ITEMS:
            raise Bad(
                f"menu {meta['label']}: {len(cmds)} entries, the host allows {MAX_MENU_ITEMS}"
            )
        menus[key] = {**meta, "items": sorted(cmds)}
    # A `children` menu is a folder on the ribbon: GUGiK holds four themes, one trigger not four.
    folders = {key: meta for key, meta in base["institutionMenus"].items() if "children" in meta}
    stale = [key for key in base["institutionMenus"] if key not in menus and key not in folders]
    if stale:
        raise Bad(f"menu '{stale[0]}' has no service in the catalogue")
    for key, meta in folders.items():
        for child in meta["children"]:
            if child not in menus:
                raise Bad(f"menu {meta['label']}: '{child}' is not a menu of services")

    known = (
        set(ids)
        | {f"tool:{t['id']}" for t in tools}
        | ({"panel"} if "panel" in base else set())
    )
    tab = base["tab"]
    if not 1 <= len(tab["groups"]) <= MAX_GROUPS:
        raise Bad(f"{len(tab['groups'])} tab groups, the host allows 1 to {MAX_GROUPS}")
    for group in tab["groups"]:
        layout = group.get("layout", "large")
        if layout not in MAX_GROUP_ITEMS:
            raise Bad(f"group {group['label']}: layout '{layout}' is not large or small")
        if "menus" in group:
            # The host's GroupDecl has no `menus`: expand the menus into items, then drop the key.
            group["items"] = [menu_decl(key, menus, folders) for key in group.pop("menus")]
        limit = MAX_GROUP_ITEMS[layout]
        if not 1 <= len(group["items"]) <= limit:
            raise Bad(
                f"group {group['label']}: {len(group['items'])} items, the host allows 1 to {limit}"
            )
        for item in group["items"]:
            check_item(group["label"], item, known)

    # Every service is on the ribbon once; this guards a service that lost its institution.
    listed = leaf_entries(tab["groups"])
    if len(listed) != len(set(listed)):
        raise Bad("a service appears in two menus")
    unlisted = [s["cmd"] for s in services if s["cmd"] not in listed]
    if unlisted:
        raise Bad(f"service not in a menu: {', '.join(unlisted[:5])}")

    for tool in tools:
        if len(tool.get("params", [])) > MAX_TOOL_PARAMS:
            raise Bad(f"tool {tool['id']}: too many parameters")
    if len(tools) > MAX_TOOLS:
        raise Bad(f"{len(tools)} tools, the host allows {MAX_TOOLS}")

    manifest = dict(base["manifest"])
    manifest["commands"] = commands
    manifest["tools"] = tools
    manifest["tab"] = tab
    manifest["i18n"] = base["i18n"]
    manifest["panel"] = base["panel"]
    check_translations(manifest)
    return manifest


def menu_strings(items, strings):
    """The label and help of every menu of a group, submenus included."""
    for item in items:
        if isinstance(item, dict):
            strings.add(item["menu"])
            if item.get("help"):
                strings.add(item["help"])
            menu_strings(item.get("items", []), strings)


def interface_strings(manifest):
    """The strings the interface shows from the manifest, minus GUGiK's own service names."""
    strings = {manifest["name"], manifest["description"], manifest["tab"]["label"]}
    for group in manifest["tab"]["groups"]:
        strings.add(group["label"])
        menu_strings(group["items"], strings)
    for command in manifest["commands"]:
        # A service's label is its official GUGiK name, the same in both languages.
        if command["id"].startswith("usluga-"):
            continue
        strings.add(command["label"])
        if command.get("help"):
            strings.add(command["help"])
    for tool in manifest["tools"]:
        strings.add(tool["name"])
        if tool.get("summary"):
            strings.add(tool["summary"])
        for param in tool.get("params", []):
            strings.add(param["label"])
            if param.get("help"):
                strings.add(param["help"])
    panel = manifest["panel"]
    for key in ("title", "help"):
        if panel.get(key):
            strings.add(panel[key])
    for item in panel["items"]:
        for key in ("label", "help", "text"):
            if item.get(key):
                strings.add(item[key])
    return strings


def check_translations(manifest):
    """Every interface string has a Polish translation, and no translation is orphaned."""
    strings = interface_strings(manifest)
    polish = manifest["i18n"].get("pl", {})
    missing = sorted(s for s in strings if s not in polish)
    if missing:
        raise Bad(
            f"{len(missing)} interface strings without a Polish translation, "
            f"starting with: {missing[0][:70]!r}"
        )
    unused = sorted(key for key in polish if key not in strings)
    if unused:
        raise Bad(
            f"{len(unused)} translations match no manifest string, "
            f"starting with: {unused[0][:70]!r}"
        )


def text(manifest):
    return json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"


def main():
    try:
        manifest = build()
    except Bad as error:
        print(f"manifest: {error}")
        return 1
    body = text(manifest)
    if "--check" in sys.argv:
        current = io.open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        if current != body:
            print("plugin.json is out of date: run python tools/manifest.py")
            return 1
        print("plugin.json is current")
        return 0
    with io.open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(body)
    print(
        f"wrote {OUT}: {len(manifest['commands'])} commands, {len(manifest['tools'])} tools"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
