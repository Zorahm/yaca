import os, re
cyr = re.compile('[\u0400-\u04FF]')
# stripped line is a comment line: //, //!, /*, /**, /*!
commentpat = re.compile(r'^(//|/\*|/\*!|/\**/*)')
exts = re.compile(r'\.(rs|tsx?|ts|jsx?|js|mjs|cjs)$')
files = set()
for root, _, fs in os.walk('.'):
    if '.git' in root:
        continue
    for f in fs:
        p = os.path.join(root, f)
        if not os.path.isfile(p) or not exts.search(f):
            continue
        try:
            with open(p, encoding='utf-8') as fh:
                lines = fh.readlines()
        except Exception:
            continue
        for l in lines:
            s = l.lstrip()
            if commentpat.match(s) and cyr.search(l):
                files.add(p)
                break
for p in sorted(files):
    print(p)
print("TOTAL", len(files))
