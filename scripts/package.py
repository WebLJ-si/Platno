#!/usr/bin/env python3
"""Turn one built program into the files a release publishes.

  package.py BINARY VERSION SUFFIX [--out dist]

Writes, for SUFFIX such as linux-x86_64, macos-universal or windows-x86_64:
  platno-VERSION-SUFFIX[.exe]         the bare program (what the in-app updater downloads)
  platno-VERSION-SUFFIX.tar.gz / .zip  the program with a short README, for people downloading by hand
"""
import argparse, pathlib, shutil, stat, tarfile, zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
README = """Platno {version} ({suffix})

Run the program:
  Linux and macOS:  ./platno        (first run: chmod +x platno)
  Windows:          platno.exe

Then open http://127.0.0.1:8787 and create the owner account. A first-run guide follows.
FFmpeg must be installed for videos that browsers cannot play directly (https://ffmpeg.org/download.html).

macOS: if the system refuses to open the file because it was downloaded, run
  xattr -d com.apple.quarantine platno
Settings, environment variables and updating: https://github.com/WebLJ-si/Platno#readme
"""


def package(binary, version, suffix, out):
    windows = suffix.startswith('windows')
    name = f'platno-{version}-{suffix}' + ('.exe' if windows else '')
    out.mkdir(parents=True, exist_ok=True)
    bare = out / name
    shutil.copyfile(binary, bare)
    bare.chmod(bare.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    inner = 'platno.exe' if windows else 'platno'
    readme = README.format(version=version, suffix=suffix)
    if windows:
        archive = out / f'platno-{version}-{suffix}.zip'
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
            z.write(binary, inner)
            z.writestr('README.txt', readme)
    else:
        archive = out / f'platno-{version}-{suffix}.tar.gz'
        with tarfile.open(archive, 'w:gz') as t:
            info = t.gettarinfo(str(binary), inner)
            info.mode, info.uid, info.gid, info.uname, info.gname = 0o755, 0, 0, '', ''
            with open(binary, 'rb') as f:
                t.addfile(info, f)
            data = readme.encode()
            note = tarfile.TarInfo('README.txt')
            note.size, note.mode = len(data), 0o644
            import io
            t.addfile(note, io.BytesIO(data))
    return bare, archive


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('binary', type=pathlib.Path)
    parser.add_argument('version')
    parser.add_argument('suffix')
    parser.add_argument('--out', type=pathlib.Path, default=ROOT / 'dist')
    args = parser.parse_args()
    for path in package(args.binary, args.version, args.suffix, args.out):
        print(path)
