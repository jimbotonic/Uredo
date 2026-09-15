# Debugging Uredo (D29, §28.2)

Rust has no `#line` directive, so debug info names the generated `target/uredo/**/*.rs` and a
debugger shows those lines. Uredo writes a provenance map beside them on every build — generated
line to Uredo line — and these helpers read it, so a breakpoint can be set in Uredo terms and a
backtrace read in them. Nothing here rewrites DWARF; §28.2 reserves that for the case where the
helpers turn out to be insufficient, and it has not arisen.

| | |
|---|---|
| `ubreak FILE.ure:LINE` | break where that Uredo line begins |
| `uwhere` | the backtrace, in Uredo locations, with the source line beside each frame |
| `ulist [N]` | the Uredo source around the current frame |

## GDB

```bash
uredo build .                       # writes target/uredo/provenance/*.json beside the generated Rust
rust-gdb target/debug/<name>
(gdb) source /path/to/compiler/debug/uredo_gdb.py
(gdb) ubreak src/main.ure:16
src/main.ure:16 is main.rs:18
Breakpoint 1 at 0x…: file src/main.rs, line 18.
(gdb) run
(gdb) uwhere
#0   c05_enum_match::Command::describe   src/main.ure:16    Quit: String::from("quit")
#1   c05_enum_match::main                src/main.ure:34    print("{}", cmd.describe())
(gdb) ulist 1
    15              Repeat(n, inner): format("{n}x {}", inner.describe())
=>  16              Quit: String::from("quit")
    17
```

`rust-gdb` loads Rust's own pretty-printers; source this on top of it. To load it every time, put
`source /path/to/uredo_gdb.py` in `~/.gdbinit`.

## LLDB

```bash
(lldb) command script import /path/to/compiler/debug/uredo_lldb.py
```

The same three commands, verified against LLDB 18.1.3 by `compiler/tests/debug.rs`. It was written
before any LLDB was available here and ran unchanged on its first real session; the one thing it
needed afterwards was trimming the Rust runtime frames LLDB walks and GDB stops before, which both
scripts now do — `uwhere` ends at the outermost frame with a Uredo location and says how many it
dropped, and `uwhere all` prints the lot.

No LLDB on the path? Ubuntu's packages extract without root, and every dependency but `liblldb`
is usually already installed:

```bash
apt-get download lldb-18 liblldb-18 python3-lldb-18      # no privileges needed
for d in *.deb; do dpkg-deb -x "$d" root/; done
export LD_LIBRARY_PATH=$PWD/root/usr/lib/x86_64-linux-gnu:$PWD/root/usr/lib/llvm-18/lib
root/usr/lib/llvm-18/bin/lldb --version
```

## VS Code

The extension talks to GDB or LLDB through a debug adapter, and both adapters accept commands to run
after the debugger starts. With CodeLLDB:

```jsonc
{
  "type": "lldb",
  "request": "launch",
  "program": "${workspaceFolder}/target/debug/<name>",
  "initCommands": ["command script import ${workspaceFolder}/compiler/debug/uredo_lldb.py"]
}
```

or, with the C/C++ extension's GDB adapter:

```jsonc
{
  "type": "cppdbg",
  "request": "launch",
  "program": "${workspaceFolder}/target/debug/<name>",
  "MIMode": "gdb",
  "setupCommands": [
    {"text": "source ${workspaceFolder}/compiler/debug/uredo_gdb.py", "description": "Uredo helpers"}
  ]
}
```

The editor's own source view still shows the generated Rust — that is what the debug info names, and
changing it needs a debug adapter of Uredo's own rather than a hook. `uwhere` and `ulist` in the
debug console are what map it back today.

## What this does not do

- **The source the debugger steps through is still the generated Rust.** Stepping is Rust's; only
  the three commands here speak Uredo.
- **`ubreak` resolves a Uredo line that generated nothing of its own** — a comment, a blank, a
  header whose body carries the code — forward to the next line that did, and says which generated
  line it chose.
- **Inspecting values is unchanged**, because the names are: a Uredo binding is a Rust binding of
  the same name, except where §6.1 makes it a raw identifier (`r#gen`), which is the name to type.
