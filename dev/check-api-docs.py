import json, io, collections, sys
d = json.load(io.open('target/doc/cli_forge.json', encoding='utf-8'))
idx = d['index']
def get(i): return idx.get(str(i)) or idx.get(i)

members = collections.defaultdict(set)
standalone = set()
for k, it in idx.items():
    if it.get('crate_id') != 0: continue
    inner = it.get('inner', {})
    if 'impl' in inner:
        imp = inner['impl']
        if imp.get('trait') is not None: continue
        for_ = imp.get('for', {})
        tname = for_.get('resolved_path', {}).get('path')
        if not tname: continue
        tname = tname.split('::')[-1]
        for mid in imp.get('items', []):
            m = get(mid)
            if m and m.get('name') and 'function' in m.get('inner', {}) and m.get('visibility') == 'public':
                members[tname].add(m['name'])

# Types, enums, traits, and the free functions re-exported at the root.
types = set()
for k, it in idx.items():
    if it.get('crate_id') != 0 or it.get('visibility') != 'public': continue
    inner = it.get('inner', {})
    kind = next(iter(inner)) if inner else None
    if kind in ('struct', 'enum', 'trait') and it.get('name'):
        types.add(it['name'])

owned = set()
for ms in members.values(): owned |= ms
for k, it in idx.items():
    if it.get('crate_id') != 0 or it.get('visibility') != 'public': continue
    if 'function' in it.get('inner', {}) and it.get('name'):
        standalone.add(it['name'])
standalone -= owned

docs = {}
for path in sys.argv[1:]:
    docs[path] = io.open(path, encoding='utf-8').read()
combined = "\n".join(docs.values())

missing = []
for t in sorted(types):
    if t not in combined: missing.append(('type', t, t))
for t in sorted(members):
    for m in sorted(members[t]):
        if m not in combined: missing.append(('method', f'{t}::{m}', m))
for f in sorted(standalone):
    if f not in combined: missing.append(('fn', f, f))

total = len(types) + sum(len(v) for v in members.values()) + len(standalone)
print(f"public items: {total}  ({len(types)} types, {sum(len(v) for v in members.values())} methods, {len(standalone)} free fns)")
print(f"checked against: {', '.join(docs)}")
if missing:
    print(f"MISSING ({len(missing)}):")
    for kind, label, _ in missing: print(f"  {kind:7} {label}")
    sys.exit(1)
print("every public item is documented")
