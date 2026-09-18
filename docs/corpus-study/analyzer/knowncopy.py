"""A mirror of `compiler/src/lower.rs::is_known_copy`, plus the corpus's own `Copy` derives.

Both corpus measurements for the §38 passing-mode questions (`optionrule.py`, `valueuse.py`) decide
"would Uredo pass this by value" with this one predicate, so a change to the compiler's rule has a
single place to follow here. `known_copy` covers scalars, shared references, tuples, fixed arrays,
the prelude table with its generic-wrapper rule, `Option`/`Result` payload recursion (D54), and the
types a repository derives `Copy` on — which is what `@derive(Copy)` would declare in Uredo code.
"""
import collections
import os

SCALARS = set("i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char () !".split())
PRELUDE = set("""Duration Instant SystemTime Ordering SocketAddr SocketAddrV4 SocketAddrV6 IpAddr
Ipv4Addr Ipv6Addr NonZeroU8 NonZeroU16 NonZeroU32 NonZeroU64 NonZeroU128 NonZeroUsize NonZeroI8
NonZeroI16 NonZeroI32 NonZeroI64 NonZeroI128 NonZeroIsize TypeId Layout PhantomData Wrapping
Saturating Range RangeInclusive RangeTo RangeFrom RangeFull""".split())
GENERIC_WRAPPERS = {"Wrapping", "Saturating", "Range", "RangeInclusive", "RangeTo", "RangeFrom"}


def split_top(s, sep):
    """Split on `sep` at bracket depth zero."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "<([":
            depth += 1
        elif ch in ">)]":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    out.append(cur)
    return [x for x in (p.strip() for p in out) if x]


def last_segment(t):
    return t.split("<")[0].split("::")[-1].strip()


def known_copy(t, copy_local, option_recursion=True):
    t = t.strip()
    if t in SCALARS:
        return True
    if t.startswith("&") and not t.startswith("&mut"):
        return True
    if t.startswith("(") and t.endswith(")"):
        return all(known_copy(p, copy_local, option_recursion) for p in split_top(t[1:-1], ","))
    if t.startswith("[") and t.endswith("]"):
        parts = split_top(t[1:-1], ";")
        if len(parts) == 2:
            return known_copy(parts[0], copy_local, option_recursion)
    seg = last_segment(t)
    if option_recursion and seg in ("Option", "Result") and "<" in t:
        inner = t[t.index("<") + 1:t.rindex(">")]
        return all(known_copy(a, copy_local, option_recursion) for a in split_top(inner, ","))
    if seg in copy_local:
        return True
    if seg in PRELUDE:
        if seg in GENERIC_WRAPPERS and "<" in t:
            return known_copy(t[t.index("<") + 1:t.rindex(">")], copy_local, option_recursion)
        return True
    return False


def copy_types_per_repo(corpus):
    """Types carrying a `Copy` derive, per repository.

    A line scan rather than a regular expression: attribute lists in real code run over several
    lines and carry comments between the derive and the item, which a backtracking pattern handles
    either wrongly or very slowly.
    """
    found = collections.defaultdict(set)
    for root, _dirs, files in os.walk(corpus):
        repo = os.path.relpath(root, corpus).split(os.sep)[0]
        for f in files:
            if not f.endswith(".rs"):
                continue
            try:
                fh = open(os.path.join(root, f), encoding="utf-8", errors="replace")
            except OSError:
                continue
            with fh:
                _scan(fh, found[repo])
    return found


def _scan(lines, out):
    pending = False          # a `Copy` derive waiting for the item it applies to
    attrs, depth = "", 0     # the attribute being collected, and its bracket depth
    for line in lines:
        t = line.strip()
        if depth or t.startswith("#["):
            attrs += t
            depth += t.count("[") + t.count("(") - t.count("]") - t.count(")")
            if depth <= 0:
                if attrs.startswith("#[derive(") and "Copy" in [
                    d.strip() for d in attrs[len("#[derive("):].split(")")[0].split(",")
                ]:
                    pending = True
                attrs, depth = "", 0
            continue
        if not t or t.startswith("//"):
            continue
        if pending:
            words = t.replace("(", " ").replace("{", " ").replace("<", " ").split()
            for i, w in enumerate(words):
                if w in ("struct", "enum", "union") and i + 1 < len(words):
                    out.add(words[i + 1].rstrip(";"))
                    break
        pending = False
