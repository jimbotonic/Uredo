"""LLDB helpers for Uredo (D29, §28.2) — the same three commands as `uredo_gdb.py`.

    (lldb) command script import compiler/debug/uredo_lldb.py
    (lldb) ubreak src/main.ure:16
    (lldb) uwhere
    (lldb) ulist

Verified against LLDB 18.1.3 by `compiler/tests/debug.rs`, which skips loudly when no `lldb` is on
the path. It had been written blind — there was no LLDB on the authoring machine for a day — and
worked unchanged on its first real run; the one thing that needed adding afterwards was trimming the
Rust runtime frames LLDB walks and GDB does not.
"""

import json
import os

import lldb


def _provenance_dirs(target):
    seen = []
    names = []
    if target is not None and target.GetExecutable().IsValid():
        spec = target.GetExecutable()
        names.append(os.path.join(spec.GetDirectory() or "", spec.GetFilename() or ""))
    for name in names:
        d = os.path.dirname(os.path.abspath(name))
        for _ in range(6):
            candidate = os.path.join(d, "target", "uredo", "provenance")
            if os.path.isdir(candidate) and candidate not in seen:
                seen.append(candidate)
            d = os.path.dirname(d)
    return seen


def _load(target):
    """`(by_rs, by_ure, dirs)` — generated line to Uredo line, and back."""
    by_rs, by_ure, dirs = {}, {}, _provenance_dirs(target)
    for d in dirs:
        for f in sorted(os.listdir(d)):
            if not f.endswith(".json"):
                continue
            try:
                with open(os.path.join(d, f)) as fh:
                    m = json.load(fh)
            except (OSError, ValueError):
                continue
            by_rs[m["rs"]] = m["lines"]
            back = by_ure.setdefault(m["ure"], {})
            for i, ure_line in enumerate(m["lines"], start=1):
                if ure_line:
                    back.setdefault(ure_line, []).append(i)
    return by_rs, by_ure, dirs


def _ure_source_line(dirs, ure_path, line):
    for d in dirs:
        package = os.path.dirname(os.path.dirname(os.path.dirname(d)))
        candidate = os.path.join(package, ure_path)
        if os.path.isfile(candidate):
            try:
                with open(candidate) as fh:
                    rows = fh.read().split("\n")
            except OSError:
                return None
            if 1 <= line <= len(rows):
                return rows[line - 1].rstrip()
    return None


def _ure_for(by_rs, rs_path, rs_line):
    for rs, lines in by_rs.items():
        if rs_path and rs_path.endswith(rs):
            if 1 <= rs_line <= len(lines) and lines[rs_line - 1]:
                return rs[: -len(".rs")] + ".ure", lines[rs_line - 1]
            return rs[: -len(".rs")] + ".ure", None
    return None, None


def ubreak(debugger, command, result, internal_dict):
    """ubreak FILE.ure:LINE — break at the generated line that Uredo line produced."""
    target = debugger.GetSelectedTarget()
    arg = command.strip()
    if ":" not in arg:
        result.SetError("usage: ubreak FILE.ure:LINE")
        return
    ure_file, _, line_text = arg.rpartition(":")
    try:
        ure_line = int(line_text)
    except ValueError:
        result.SetError("usage: ubreak FILE.ure:LINE")
        return
    _, by_ure, _ = _load(target)
    if not by_ure:
        result.SetError("no provenance maps found; build with `uredo build` first")
        return
    for ure, back in by_ure.items():
        if ure == ure_file or ure.endswith("/" + ure_file) or os.path.basename(ure) == ure_file:
            lines = back.get(ure_line) or sorted(l for k, ls in back.items() if k >= ure_line for l in ls)
            if not lines:
                continue
            rs = os.path.basename(ure)[: -len(".ure")] + ".rs"
            bp = target.BreakpointCreateByLocation(rs, min(lines))
            result.AppendMessage("%s:%d is %s:%d (%d location(s))" % (ure_file, ure_line, rs, min(lines), bp.GetNumLocations()))
            return
    result.SetError("no generated line for %s:%d" % (ure_file, ure_line))


def uwhere(debugger, command, result, internal_dict):
    """uwhere — the backtrace in Uredo terms, ending at the outermost frame that has one.

    Everything past that is the Rust runtime getting to `main`, which is nineteen frames on a
    trivial program and tells a Uredo reader nothing; the count of what was dropped is printed so
    the omission is visible. `uwhere all` prints the whole stack.
    """
    show_all = command.strip() == "all"
    target = debugger.GetSelectedTarget()
    by_rs, _, dirs = _load(target)
    thread = target.GetProcess().GetSelectedThread()

    rows, last_uredo = [], -1
    for n in range(thread.GetNumFrames()):
        frame = thread.GetFrameAtIndex(n)
        entry = frame.GetLineEntry()
        spec = entry.GetFileSpec()
        rs_path = os.path.join(spec.GetDirectory() or "", spec.GetFilename() or "")
        ure, ure_line = _ure_for(by_rs, rs_path, entry.GetLine())
        if ure is not None and ure_line is not None:
            text = _ure_source_line(dirs, ure, ure_line)
            where = "%s:%d" % (ure, ure_line) + ("    %s" % text.strip() if text else "")
            last_uredo = n
        else:
            where = "%s:%d" % (rs_path, entry.GetLine())
        rows.append("#%-3d %-40s %s" % (n, frame.GetFunctionName() or "??", where))

    shown = len(rows) if show_all or last_uredo < 0 else last_uredo + 1
    for row in rows[:shown]:
        result.AppendMessage(row)
    if shown < len(rows):
        result.AppendMessage("     … %d more frame(s) with no Uredo source; `uwhere all` for the lot" % (len(rows) - shown))


def ulist(debugger, command, result, internal_dict):
    """ulist [N] — the Uredo source around the current frame, N lines either side (default 3)."""
    span = int(command) if command.strip().isdigit() else 3
    target = debugger.GetSelectedTarget()
    by_rs, _, dirs = _load(target)
    frame = target.GetProcess().GetSelectedThread().GetSelectedFrame()
    entry = frame.GetLineEntry()
    spec = entry.GetFileSpec()
    rs_path = os.path.join(spec.GetDirectory() or "", spec.GetFilename() or "")
    ure, ure_line = _ure_for(by_rs, rs_path, entry.GetLine())
    if ure is None or ure_line is None:
        result.SetError("no Uredo source for %s:%d" % (rs_path, entry.GetLine()))
        return
    for n in range(max(1, ure_line - span), ure_line + span + 1):
        text = _ure_source_line(dirs, ure, n)
        if text is None:
            continue
        result.AppendMessage("%s%4d  %s" % ("=>" if n == ure_line else "  ", n, text))


def __lldb_init_module(debugger, internal_dict):
    for name in ("ubreak", "uwhere", "ulist"):
        debugger.HandleCommand("command script add -f uredo_lldb.%s %s" % (name, name))
    print("uredo: ubreak, uwhere, ulist loaded (D29, §28.2)")
