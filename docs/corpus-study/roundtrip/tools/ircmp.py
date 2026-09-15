#!/usr/bin/env python3
"""Per-function comparison of two LLVM IR files (raw or normalized).
For each `define`: instruction count (labels/comments excluded), opcode sequence, alloca byte sizes.
status: same_opcodes | same_counts_diff_opcodes | DIFF | ONLY-ONE-SIDE. Exit 1 if any function is not same_opcodes or no functions found."""
import re,sys
def crate_norm(n): return re.sub(r'Xs[0-9a-z]*_','Xs_',re.sub(r'B[0-9a-zA-Z]+_','B_',re.sub(r'Cs[A-Za-z0-9]+_[0-9]+(piece|p[0-9]_(orig|gen))','CsX_piece',re.sub(r'17h[0-9a-f]{16}E','17hHE',n))))
def funcs(path):
    out={}; name=None
    for line in open(path):
        m=re.match(r'define .*?@("?[^("]+"?)\(',line)
        if m: name=crate_norm(m.group(1)); ops=[]; allocs=[]; continue
        if name is None: continue
        if line.startswith('}'):
            if name in out: raise SystemExit(f"duplicate normalized function name {name}")
            out[name]=(ops,allocs); name=None; continue
        s=line.split(';',1)[0].strip()
        if not s or re.match(r'^[A-Za-z0-9_.$"]+:$',s): continue   # label
        m=re.match(r'(?:%[^ ]+ = )?(\w+)',s)
        op=m.group(1) if m else s.split()[0]
        if op=='call' or op=='invoke' or op=='tail':
            cm=re.search(r'@("?[^(" ]+"?)',s); op=op+' '+(crate_norm(cm.group(1)) if cm else '?')
        ops.append(op)
        if op=='alloca':
            am=re.search(r'alloca \[(\d+) x i8\]',s); allocs.append(int(am.group(1)) if am else -1)
    return out
def short(n): return re.sub(r'^_R[A-Za-z0-9_]*?piece','',n)[:70]
a=funcs(sys.argv[1]); b=funcs(sys.argv[2])
same=diff=only=counts_only=0
for k in sorted(set(a)|set(b)):
    if k not in a or k not in b: only+=1; print(f"{short(k)}\t{len(a.get(k,([],[]))[0])}\t{len(b.get(k,([],[]))[0])}\t\t\tONLY-ONE-SIDE"); continue
    oa,aa=a[k]; ob,ab=b[k]
    if oa==ob and aa==ab: same+=1
    elif len(oa)==len(ob) and aa==ab: counts_only+=1; print(f"{short(k)}\t{len(oa)}\t{len(ob)}\t{sum(aa)}\t{sum(ab)}\tsame_counts_diff_opcodes")
    else: diff+=1; print(f"{short(k)}\t{len(oa)}\t{len(ob)}\t{sum(aa)}B\t{sum(ab)}B\tDIFF")
print(f"SUMMARY functions={len(set(a)|set(b))} same_opcodes={same} same_counts_diff_opcodes={counts_only} diff={diff} only_one_side={only} instr_total_orig={sum(len(v[0]) for v in a.values())} instr_total_gen={sum(len(v[0]) for v in b.values())} alloca_bytes_orig={sum(sum(v[1]) for v in a.values())} alloca_bytes_gen={sum(sum(v[1]) for v in b.values())}")
sys.exit(0 if (same>0 and diff==0 and only==0 and counts_only==0) else 1)
