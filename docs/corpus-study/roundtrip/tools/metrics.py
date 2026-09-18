#!/usr/bin/env python3
"""Size and complexity metrics for .rs and .ure files, computed with ONE tokenizer so the versions are comparable.
Usage: metrics.py file [file...]  -> TSV rows"""
import re,sys,os
PUNCT=r'::<|::|->|=>|\.\.=|\.\.|&&|\|\||==|!=|<=|>=|\+=|-=|\*=|/=|%=|<<|>>|[-+*/%=<>!&|^~?@#$:;,.(){}\[\]]'
TOK=re.compile(r"r#\"[\s\S]*?\"#|r\"[^\"]*\"|b?\"(?:\\.|[^\"\\])*\"|b?'(?:\\u\{[0-9A-Fa-f]+\}|\\x[0-9A-Fa-f]{2}|\\.|[^'\\\n])'|'[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*!?|0x[0-9A-Fa-f_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?[A-Za-z0-9]*|"+PUNCT)
def strip_comments(src,lang):
    """`src` with comments removed and every string literal left intact.

    One scan over the whole text rather than one per line, because a raw string spans lines and its
    `#` and `//` are not comments. A line-at-a-time scanner cut `const X = r#"` at the `#`, and the
    body of the literal was then read as code (found 2026-09-13, measuring a tool that embeds a Rust
    snippet). Newlines are preserved so the line count is unchanged.
    """
    out=[]; i=0; n=len(src)
    while i<n:
        c=src[i]
        # raw string: r#*" … "#*  (the hash count has to match)
        if c=='r':
            j=i+1
            while j<n and src[j]=='#': j+=1
            if j<n and src[j]=='"':
                hashes=j-i-1
                out.append(src[i:j+1]); k=j+1
                closer='"'+'#'*hashes
                stop=src.find(closer,k)
                if stop<0: stop=n-len(closer)
                out.append(src[k:stop+len(closer)])
                i=stop+len(closer); continue
        if c=='"':
            out.append(c); i+=1
            while i<n:
                out.append(src[i])
                if src[i]=='\\' and i+1<n:
                    out.append(src[i+1]); i+=2; continue
                if src[i]=='"': i+=1; break
                i+=1
            continue
        if lang=='rs' and src.startswith('/*',i):
            stop=src.find('*/',i+2)
            stop=n if stop<0 else stop+2
            out.append('\n'*src.count('\n',i,stop))     # keep the line count
            i=stop; continue
        if src.startswith('//',i) or (lang!='rs' and c=='#'):
            stop=src.find('\n',i)
            i=n if stop<0 else stop
            continue
        out.append(c); i+=1
    return ''.join(out)
def split_tests(src,lang):
    if lang=='rs':
        i=src.find('#[cfg(test)]'); return (src if i<0 else src[:i])
    i=src.find('@cfg(test)'); return (src if i<0 else src[:i])
def match_arms_ure(lines):
    arms=0; stack=[]
    for l in lines:
        if not l.strip(): continue
        ind=len(l)-len(l.lstrip(' '))
        while stack and ind<=stack[-1]: stack.pop()
        if re.search(r'\bmatch\b.*:\s*$',l): stack.append(ind); continue
        if stack and ind==stack[-1]+4: arms+=1
    return arms
LITERAL=re.compile(r'^(?:r\#*"|b?"|b?\'.*\'$)')
def blank_literals(code,toks):
    """`code` with every string and char literal replaced by `""`.

    Annotations are a property of the code, not of the strings inside it: an English possessive
    counts as a lifetime and an `&` in a message as a borrow. Pairing quotes with a regex gets this
    wrong on `r#"..."#`, whose body may hold a quote of its own, so the tokenizer above — which
    already knows every literal form — is what decides where a literal starts and ends. Lifetimes
    are left alone: they are what the tick count is for."""
    out=[]; last=0
    for m in toks:
        t=m.group(0)
        if LITERAL.match(t):
            out.append(code[last:m.start()]); out.append('""'); last=m.end()
    out.append(code[last:])
    return ''.join(out)
def metrics(path):
    lang='ure' if path.endswith('.ure') else 'rs'
    raw=open(path).read(); body=split_tests(raw,lang); code=strip_comments(body,lang)
    lines=[l for l in code.split('\n') if l.strip()]
    toks_with_pos=list(TOK.finditer(code))
    toks=[m.group(0) for m in toks_with_pos]
    ident=[t for t in toks if re.match(r'[A-Za-z_]',t) and not t.startswith(("'",'"','b"'))]
    punct=[t for t in toks if re.match(PUNCT+r'$',t)]
    nonws=len(re.sub(r'\s','',code))
    # cyclomatic
    kw=lambda k: len(re.findall(r'\b'+k+r'\b',code))
    # Annotations are a property of the code, not of the strings inside it: blank every string
    # literal first, or an English possessive is counted as a lifetime and an `&` in a message as a
    # borrow. Found 2026-09-13 while measuring `tools/`, whose prose-heavy literals made it visible;
    # the lifetime pattern also has to admit `'static`, which `'[a-z]\b` never matched.
    bare=blank_literals(code,toks_with_pos)
    akw=lambda k: len(re.findall(r'\b'+k+r'\b',bare))
    ticks=len(re.findall(r"'[a-z_]+\b",bare))
    if lang=='rs':
        arms=code.count('=>')
        nest=0; d=0
        for c in code:
            if c=='{': d+=1; nest=max(nest,d)
            elif c=='}': d-=1
        annot=bare.count('&')+akw('mut')+ticks
    else:
        arms=match_arms_ure(code.split('\n'))
        nest=max((len(l)-len(l.lstrip(' ')))//4 for l in lines) if lines else 0
        annot=bare.count('&')+akw('take')+akw('inout')+akw('var')+ticks
    matches=kw('match')
    # logical operators: exclude closure parameter lists `|x|` and `||` in Rust
    logic=re.sub(r'([(,=])\s*\|\|',r'\1',code) if lang=='rs' else code
    logic=re.sub(r'([(,=])\s*\|[^|\n]*\|',r'\1',logic) if lang=='rs' else logic
    # `?` only as an operator: strip string and char literals first
    ops=re.sub(r"'[A-Za-z_][A-Za-z0-9_]*\b(?!')",'',code)   # lifetimes first
    ops=re.sub(r'r#\"[\s\S]*?\"#|r\"[^\"]*\"|b?\"(?:\\.|[^\"\\])*\"|b?\'(?:\\u\{[0-9A-Fa-f]+\}|\\x[0-9A-Fa-f]{2}|\\.|[^\'\\\n])\'','',ops)
    ands=logic.count('&&'); ors=logic.count('||'); qs=ops.count('?')
    cyclo=1+kw('if')+kw('while')+kw('for')+ands+ors+qs+max(0,arms-matches)
    if os.environ.get('METRICS_DEBUG'): print(f"# {path}: if={kw('if')} while={kw('while')} for={kw('for')} and={ands} or={ors} q={qs} arms={arms} matches={matches}",file=sys.stderr)
    return dict(file=path,lang=lang,lines=len(lines),nonws_chars=nonws,tokens=len(toks),ident_tokens=len(ident),punct_tokens=len(punct),cyclomatic=cyclo,max_nesting=nest,annotations=annot,fns=kw('fn'))
if __name__=='__main__':
    rows=[metrics(p) for p in sys.argv[1:]]
    print('\t'.join(rows[0].keys()))
    for r in rows: print('\t'.join(str(v) for v in r.values()))
