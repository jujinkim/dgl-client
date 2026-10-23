#!/usr/bin/env python3
"""Build a pinned test-only tmux in .tools; never install over the user's tmux."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import urllib.request

checksums = {
    '3.3a': 'e4fd347843bd0772c4f48d6dde625b0b109b7a380ff15db21e97c11a4dcdf93f',
    '3.7c': '7c60cae9a0e25288e2e24750aafc9e8800fc7fd4555e447e1b29ee4201cfb3bf',
}
parser = argparse.ArgumentParser()
parser.add_argument('version', choices=checksums)
version = parser.parse_args().version
root = Path(__file__).resolve().parents[1] / '.tools' / ('tmux-' + version)
root.mkdir(parents=True, exist_ok=True)
archive = root / 'source.tar.gz'
if not archive.exists():
    urllib.request.urlretrieve(f'https://github.com/tmux/tmux/releases/download/{version}/tmux-{version}.tar.gz', archive)
if hashlib.sha256(archive.read_bytes()).hexdigest() != checksums[version]:
    raise SystemExit('tmux source checksum mismatch')
with tarfile.open(archive) as source:
    members = source.getmembers()
    for member in members:
        if not (member.isfile() or member.isdir()) or not (root / member.name).resolve().is_relative_to(root.resolve()):
            raise SystemExit('Unexpected tmux archive entry')
    source.extractall(root, members=members)
build = root / ('tmux-' + version)
env = os.environ.copy()
if Path('/opt/homebrew/opt/libevent').exists():
    env.setdefault('CPPFLAGS', '-I/opt/homebrew/opt/libevent/include -I/opt/homebrew/opt/ncurses/include')
    env.setdefault('LDFLAGS', '-L/opt/homebrew/opt/libevent/lib -L/opt/homebrew/opt/ncurses/lib')
flags = ['./configure', '--disable-utf8proc']
if version == '3.7c':
    flags.append('--disable-jemalloc')
subprocess.run(flags, cwd=build, env=env, check=True)
subprocess.run(['make', '-j2'], cwd=build, check=True)
print('Test tmux:', build / 'tmux')
