import io, os, re, glob, sys

files = ['README.md','CHANGELOG.md','docs/README.md','docs/GUIDE.md','docs/OUTPUT.md',
         'docs/COMMANDS.md','docs/RECIPES.md','docs/API.md','dev/DIRECTIVES.md',
         'dev/ROADMAP.md'] + sorted(glob.glob('docs/release/*.md'))
link = re.compile(r'\[([^\]]*)\]\(([^)]+)\)')
bad = []

def anchors_of(path):
    out = set()
    raw = io.open(path, encoding='utf-8').read()
    for line in raw.split('\n'):
        m = re.match(r'#{1,6}\s+(.*)', line.strip())
        if not m: continue
        t = re.sub(r'`', '', m.group(1).strip())
        t = re.sub(r'<[^>]+>', '', t)
        t = re.sub(r'&[a-z]+;', '', t).lower()
        t = re.sub(r'[^\w\s-]', '', t)
        out.add(t.strip().replace(' ', '-'))
    for m in re.finditer(r'id="([^"]+)"', raw):
        out.add(m.group(1))
    return out

cache = {}
for f in files:
    if not os.path.exists(f):
        bad.append((f, '<file itself missing>')); continue
    base = os.path.dirname(f)
    for _, target in link.findall(io.open(f, encoding='utf-8').read()):
        if target.startswith(('http://', 'https://', 'mailto:')): continue
        path, _, anchor = target.partition('#')
        resolved = os.path.normpath(os.path.join(base, path)) if path else f
        if path and not os.path.exists(resolved):
            bad.append((f, target)); continue
        if anchor and resolved.endswith('.md'):
            if resolved not in cache: cache[resolved] = anchors_of(resolved)
            if anchor not in cache[resolved]: bad.append((f, target))

print(f"checked {len(files)} files")
if bad:
    print(f"BROKEN ({len(bad)}):")
    for f, t in bad: print(f"  {f} -> {t}")
    sys.exit(1)
print("every relative link and anchor resolves")
