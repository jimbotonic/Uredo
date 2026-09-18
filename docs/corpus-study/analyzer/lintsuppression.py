"""Where and how often real Rust suppresses lints, per repository and per position (§23 review).

Counts an attribute at the line it begins on, collecting the several lines a real one spans until its
brackets balance; an attribute written mid-line is not counted, which is the whole of the difference
from a plain `ripgrep` count of the pattern (1,160 here against 1,201). A name containing `=` or
opening a quote is a `reason = "…"` clause, not a lint. Repositories with no suppression at all are
listed too, since dropping them silently moved the median.
"""
import collections, os, re, sys
corpus = sys.argv[1]
pat = re.compile(r'#(!?)\[(allow|expect|warn|deny|forbid)\(([^\]]*)\)\]')
per_repo = collections.Counter()
kind = collections.Counter()
names = collections.Counter()
tool = collections.Counter()
inner = collections.Counter()
position = collections.Counter()
files = 0
for root, _d, fs in os.walk(corpus):
    repo = os.path.relpath(root, corpus).split(os.sep)[0]
    for f in fs:
        if not f.endswith('.rs'):
            continue
        files += 1
        try:
            src = open(os.path.join(root, f), encoding='utf-8', errors='replace').read()
        except OSError:
            continue
        lines = src.split('\n')
        for i, line in enumerate(lines):
            start = re.match(r'\s*#(!?)\[(allow|expect|warn|deny|forbid)\(', line)
            if not start:
                continue
            # an attribute list may run over several lines; collect until the brackets balance
            text, j, depth = '', i, 0
            while j < len(lines):
                text += lines[j]
                depth = text.count('[') + text.count('(') - text.count(']') - text.count(')')
                if depth <= 0:
                    break
                j += 1
            m = re.match(r'\s*#(!?)\[(allow|expect|warn|deny|forbid)\((.*)\)\]', text, re.S)
            if m:
                bang, k, args = m.groups()
            else:
                bang, k, args = start.group(1), start.group(2), ''
            if True:
                kind[k] += 1
                if k != 'allow' and k != 'expect':
                    continue
                per_repo[repo] += 1
                inner['crate/module inner (#![…])' if bang else 'item (#[…])'] += 1
                for a in args.split(','):
                    a = a.strip()
                    if not a or '=' in a or a.startswith('"'):
                        continue          # `reason = "…"` is not a lint name
                    names[a] += 1
                    tool[a.split('::')[0] if '::' in a else 'rustc'] += 1
                if not bang:
                    # what does the attribute sit on? look at the next non-attribute, non-comment line
                    j = j + 1
                    while j < len(lines):
                        t = lines[j].strip()
                        if t == '' or t.startswith('#[') or t.startswith('#!') or t.startswith('//') or t.startswith(')') or t.startswith(']'):
                            j += 1
                            continue
                        break
                    nxt = lines[j].strip() if j < len(lines) else ''
                    if re.match(r'(pub(\([^)]*\))?\s+)?(async\s+|const\s+|unsafe\s+|extern\s+)*fn ', nxt):
                        position['function'] += 1
                    elif re.match(r'(pub(\([^)]*\))?\s+)?(struct|enum|union|trait|type|mod|impl|use|static|const)\b', nxt):
                        position['other item'] += 1
                    elif re.match(r'(let|match|for|while|if|loop|return|unsafe)\b', nxt):
                        position['statement or expression'] += 1
                    elif re.match(r'[A-Za-z_][A-Za-z0-9_]*\s*:', nxt):
                        position['struct field or variant'] += 1
                    else:
                        position['other'] += 1
print("Rust files scanned:", files)
print("\nby attribute kind:")
for k, v in kind.most_common():
    print("  %-8s %6d" % (k, v))
print("\nallow/expect by position:")
for k, v in inner.most_common():
    print("  %-26s %6d" % (k, v))
for k, v in position.most_common():
    print("    item sits on %-24s %6d" % (k, v))
print("\nallow/expect per repository (per 1,000 Rust lines):")
for repo in sorted(os.listdir(corpus)):
    if not os.path.isdir(os.path.join(corpus, repo)):
        continue
    per_repo.setdefault(repo, 0)
for repo, v in per_repo.most_common():
    n = 0
    for root, _d, fs in os.walk(os.path.join(corpus, repo)):
        for f in fs:
            if f.endswith('.rs'):
                try:
                    n += sum(1 for _ in open(os.path.join(root, f), encoding='utf-8', errors='replace'))
                except OSError:
                    pass
    print("  %-12s %6d   %.2f" % (repo, v, 1000 * v / max(n, 1)))
print("\nnamespace of the suppressed lint:")
for k, v in tool.most_common(8):
    print("  %-10s %6d" % (k, v))
print("\ntop suppressed lints:")
for k, v in names.most_common(20):
    print("  %5d  %s" % (v, k))
