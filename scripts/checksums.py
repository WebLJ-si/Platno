#!/usr/bin/env python3
"""Write SHA256SUMS for every file in a folder (the format `sha256sum -c` and the in-app updater read)."""
import hashlib, pathlib, sys

folder = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else 'dist')
lines = []
for path in sorted(p for p in folder.iterdir() if p.is_file() and p.name != 'SHA256SUMS'):
    digest = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            digest.update(chunk)
    lines.append(f'{digest.hexdigest()}  {path.name}')
(folder / 'SHA256SUMS').write_text('\n'.join(lines) + '\n')
print('\n'.join(lines))
