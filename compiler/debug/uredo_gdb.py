"""GDB helpers for Uredo (D29, §28.2).

Rust has no `#line` directive, so debug info names the generated `target/uredo/**/*.rs` and a
debugger shows those lines. Uredo writes a provenance map beside them on every build — generated
line to Uredo line — and these helpers read it, so a breakpoint can be set in Uredo terms and a
backtrace can be read in them.

Only public GDB extension APIs are used; nothing rewrites DWARF.

    (gdb) source compiler/debug/uredo_gdb.py
    (gdb) ubreak src/main.ure:16      # break where that Uredo line begins
    (gdb) uwhere                      # the backtrace, in Uredo locations
    (gdb) ulist                       # the Uredo source around the current frame

`rust-gdb` loads Rust's own pretty-printers; source this on top of it.
"""

import json
import os
import gdb


def _provenance_dirs():
    """Every `target/uredo/provenance` above the objfiles GDB has loaded.

    The program is under `<package>/target/debug/<name>` and the maps under
    `<package>/target/uredo/provenance`, so the package directory is what both have in common.
    """
    seen = []
    names = [o.filename for o in gdb.objfiles() if o.filename]
    space = gdb.current_progspace()
    if space is not None and space.filename:
        names.append(space.filename)
    for name in names:
        d = os.path.dirname(os.path.abspath(name))
        for _ in range(6):
            candidate = os.path.join(d, "target", "uredo", "provenance")
            if os.path.isdir(candidate) and candidate not in seen:
                seen.append(candidate)
            d = os.path.dirname(d)
    return seen


class Maps:
    """The provenance of one build: generated line to Uredo line, and back."""

    def __init__(self):
        self.by_rs = {}      # 'src/main.rs' -> [ure_line per generated line, 1-based]
        self.by_ure = {}     # 'src/main.ure' -> {ure_line: [generated lines]}
        self.loaded = []

    def load(self):
        self.__init__()
        for d in _provenance_dirs():
            for f in sorted(os.listdir(d)):
                if not f.endswith(".json"):
                    continue
                path = os.path.join(d, f)
                try:
                    with open(path) as fh:
                        m = json.load(fh)
                except (OSError, ValueError):
                    continue
                self.by_rs[m["rs"]] = m["lines"]
                back = self.by_ure.setdefault(m["ure"], {})
                for i, ure_line in enumerate(m["lines"], start=1):
                    if ure_line:
                        back.setdefault(ure_line, []).append(i)
                self.loaded.append(path)
        return self

    def generated_for(self, ure_file, ure_line):
        """The first generated line of a Uredo line, and the generated file it is in."""
        for ure, back in self.by_ure.items():
            if ure == ure_file or ure.endswith("/" + ure_file) or os.path.basename(ure) == ure_file:
                lines = back.get(ure_line)
                if lines:
                    rs = ure[: -len(".ure")] + ".rs"
                    return rs, min(lines)
                # a line that generated nothing of its own — a comment, a blank, a `:` header
                # whose body carries the code — resolves forward to the next line that did
                later = sorted(l for k, ls in back.items() if k >= ure_line for l in ls)
                if later:
                    rs = ure[: -len(".ure")] + ".rs"
                    return rs, later[0]
        return None, None

    def ure_for(self, rs_path, rs_line):
        """The Uredo file and line a generated location came from, or `(None, None)`."""
        for rs, lines in self.by_rs.items():
            if rs_path.endswith(rs):
                if 1 <= rs_line <= len(lines) and lines[rs_line - 1]:
                    return rs[: -len(".rs")] + ".ure", lines[rs_line - 1]
                return rs[: -len(".rs")] + ".ure", None
        return None, None


MAPS = Maps()


def _ure_source_line(ure_path, line):
    """The text of a Uredo line, looked for beside the generated tree."""
    for d in _provenance_dirs():
        package = os.path.dirname(os.path.dirname(os.path.dirname(d)))  # …/target/uredo/provenance
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


class UBreak(gdb.Command):
    """ubreak FILE.ure:LINE — break at the generated line that Uredo line produced."""

    def __init__(self):
        super().__init__("ubreak", gdb.COMMAND_BREAKPOINTS)

    def invoke(self, arg, from_tty):
        arg = arg.strip()
        if ":" not in arg:
            raise gdb.GdbError("usage: ubreak FILE.ure:LINE")
        ure_file, _, line_text = arg.rpartition(":")
        try:
            ure_line = int(line_text)
        except ValueError:
            raise gdb.GdbError("usage: ubreak FILE.ure:LINE")
        MAPS.load()
        if not MAPS.loaded:
            raise gdb.GdbError("no provenance maps found; build with `uredo build` first")
        rs, rs_line = MAPS.generated_for(ure_file, ure_line)
        if rs is None:
            raise gdb.GdbError("no generated line for %s:%d in %s" % (ure_file, ure_line, ", ".join(MAPS.loaded)))
        location = "%s:%d" % (os.path.basename(rs), rs_line)
        gdb.write("%s:%d is %s\n" % (ure_file, ure_line, location))
        gdb.execute("break " + location)


class UWhere(gdb.Command):
    """uwhere [all] — the backtrace in Uredo terms, ending at the outermost frame that has one.

    Everything past that is the Rust runtime getting to `main` and tells a Uredo reader nothing; the
    count of what was dropped is printed so the omission is visible. `uwhere all` prints the lot.
    """

    def __init__(self):
        super().__init__("uwhere", gdb.COMMAND_STACK)

    def invoke(self, arg, from_tty):
        MAPS.load()
        show_all = arg.strip() == "all"
        rows, last_uredo = [], -1
        frame = gdb.newest_frame()
        n = 0
        while frame is not None:
            sal = frame.find_sal()
            name = frame.name() or "??"
            where = ""
            if sal is not None and sal.symtab is not None:
                rs_path = sal.symtab.filename
                ure, ure_line = MAPS.ure_for(rs_path, sal.line)
                if ure is not None and ure_line is not None:
                    text = _ure_source_line(ure, ure_line)
                    where = "%s:%d" % (ure, ure_line)
                    if text:
                        where += "    %s" % text.strip()
                    last_uredo = n
                elif ure is not None:
                    where = "%s (generated line %d has no Uredo source)" % (ure, sal.line)
                else:
                    where = "%s:%d" % (rs_path, sal.line)
            rows.append("#%-3d %-40s %s\n" % (n, name, where))
            frame = frame.older()
            n += 1

        shown = len(rows) if show_all or last_uredo < 0 else last_uredo + 1
        for row in rows[:shown]:
            gdb.write(row)
        if shown < len(rows):
            gdb.write("     … %d more frame(s) with no Uredo source; `uwhere all` for the lot\n" % (len(rows) - shown))


class UList(gdb.Command):
    """ulist [N] — the Uredo source around the current frame, N lines either side (default 3)."""

    def __init__(self):
        super().__init__("ulist", gdb.COMMAND_FILES)

    def invoke(self, arg, from_tty):
        span = int(arg) if arg.strip().isdigit() else 3
        MAPS.load()
        frame = gdb.selected_frame()
        sal = frame.find_sal()
        if sal is None or sal.symtab is None:
            raise gdb.GdbError("no source location for this frame")
        ure, ure_line = MAPS.ure_for(sal.symtab.filename, sal.line)
        if ure is None or ure_line is None:
            raise gdb.GdbError("no Uredo source for %s:%d" % (sal.symtab.filename, sal.line))
        for n in range(max(1, ure_line - span), ure_line + span + 1):
            text = _ure_source_line(ure, n)
            if text is None:
                continue
            gdb.write("%s%4d  %s\n" % ("=>" if n == ure_line else "  ", n, text))


UBreak()
UWhere()
UList()
gdb.write("uredo: ubreak, uwhere, ulist loaded (D29, §28.2)\n")
