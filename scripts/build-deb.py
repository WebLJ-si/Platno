#!/usr/bin/env python3
"""Build platno_VERSION_ARCH.deb from a built Linux program (needs dpkg-deb).

  build-deb.py BINARY VERSION amd64|arm64 [--out dist]

The package installs /usr/bin/platno, a systemd service running as the system user `platno`, and
/etc/default/platno for settings. It depends only on ffmpeg because the program is a static binary.
"""
import argparse, pathlib, shutil, stat, subprocess, tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
FILES = ROOT / 'packaging' / 'debian'


def build(binary, version, arch, out):
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        stage = pathlib.Path(tmp) / 'pkg'
        (stage / 'usr/bin').mkdir(parents=True)
        (stage / 'lib/systemd/system').mkdir(parents=True)
        (stage / 'etc/default').mkdir(parents=True)
        (stage / 'usr/share/doc/platno').mkdir(parents=True)
        (stage / 'DEBIAN').mkdir()
        shutil.copyfile(binary, stage / 'usr/bin/platno')
        (stage / 'usr/bin/platno').chmod(0o755)
        shutil.copyfile(FILES / 'platno.service', stage / 'lib/systemd/system/platno.service')
        shutil.copyfile(FILES / 'platno.default', stage / 'etc/default/platno')
        shutil.copyfile(ROOT / 'README.md', stage / 'usr/share/doc/platno/README.md')
        size = sum(f.stat().st_size for f in stage.rglob('*') if f.is_file()) // 1024 + 1
        (stage / 'DEBIAN/control').write_text((FILES / 'control').read_text().format(version=version, arch=arch, size=size))
        (stage / 'DEBIAN/conffiles').write_text('/etc/default/platno\n')
        for script in ('postinst', 'prerm', 'postrm'):
            target = stage / 'DEBIAN' / script
            shutil.copyfile(FILES / script, target)
            target.chmod(0o755)
        for path in [stage, *stage.rglob('*')]:
            if path.is_dir():
                path.chmod(0o755)
        target = out / f'platno_{version}_{arch}.deb'
        subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(stage), str(target)], check=True, stdout=subprocess.DEVNULL)
        return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('binary', type=pathlib.Path)
    parser.add_argument('version')
    parser.add_argument('arch', choices=['amd64', 'arm64'])
    parser.add_argument('--out', type=pathlib.Path, default=ROOT / 'dist')
    args = parser.parse_args()
    print(build(args.binary, args.version, args.arch, args.out))
