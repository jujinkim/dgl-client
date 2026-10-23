#!/usr/bin/env python3
"""Run SSHJ tests against a disposable OpenSSH + private tmux socket.

No user SSH config, authorized_keys, or existing tmux server is modified.
All private keys are generated at runtime and removed on exit.
"""
import argparse
import getpass
import os
from pathlib import Path
import secrets
import shlex
import shutil
import socket
import subprocess
import tempfile
import time


def run(args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, encoding="utf-8", **kwargs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--tmux', default=shutil.which('tmux'))
    parser.add_argument('--command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.tmux:
        raise SystemExit('tmux is required')
    tmux = str(Path(args.tmux).resolve())
    with tempfile.TemporaryDirectory(prefix='dgl-integration-') as folder:
        root = Path(folder)
        os.chmod(root, 0o700)
        for name, keytype in [('host', 'ed25519'), ('client', 'ed25519'), ('rsa', 'rsa')]:
            run(['ssh-keygen', '-q', '-t', keytype, '-N', '', '-f', str(root / name)])
        passphrase = secrets.token_urlsafe(24)
        run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', passphrase, '-f', str(root / 'encrypted')])
        (root / 'authorized_keys').write_text(''.join((root / (name + '.pub')).read_text(encoding="utf-8") for name in ['client', 'rsa', 'encrypted']), encoding="utf-8")
        fingerprint = run(['ssh-keygen', '-lf', str(root / 'host.pub'), '-E', 'sha256']).stdout.split()[1]
        (root / 'bin').mkdir()
        (root / 'tmux.conf').write_text('set -g default-shell /bin/sh\nset -g default-command /bin/sh\nset -g automatic-rename off\nset-environment -g LANG en_US.UTF-8\nset-environment -g LC_ALL en_US.UTF-8\n', encoding="utf-8")
        wrapper = root / 'bin/tmux'
        wrapper.write_text('#!/bin/sh\nexec ' + shlex.quote(tmux) + ' -S ' + shlex.quote(str(root / 'tmux.socket')) + ' -f ' + shlex.quote(str(root / 'tmux.conf')) + ' "$@"\n', encoding="utf-8")
        wrapper.chmod(0o700)
        dispatcher = root / 'dispatch.sh'
        dispatcher.write_text('#!/bin/sh\nunset TMUX\nexport PATH=' + shlex.quote(str(root / 'bin') + ':/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin') + '\nif [ -n "$SSH_ORIGINAL_COMMAND" ]; then exec /bin/sh -c "$SSH_ORIGINAL_COMMAND"; else exec /bin/sh -i; fi\n', encoding="utf-8")
        dispatcher.chmod(0o700)
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', 0))
            port = probe.getsockname()[1]
        config = root / 'sshd_config'
        config.write_text(f'''Port {port}
ListenAddress 127.0.0.1
HostKey {root}/host
PidFile {root}/sshd.pid
AuthorizedKeysFile {root}/authorized_keys
StrictModes no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
UsePAM no
PermitRootLogin no
AllowUsers {getpass.getuser()}
AllowTcpForwarding no
X11Forwarding no
PermitTunnel no
ForceCommand /bin/sh {dispatcher}
LogLevel ERROR
''', encoding="utf-8")
        sshd = shutil.which('sshd') or '/usr/sbin/sshd'
        with (root / 'sshd.log').open('w+', encoding='utf-8') as log:
            process = subprocess.Popen([sshd, '-D', '-e', '-f', str(config)], stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if process.poll() is not None:
                        log.seek(0)
                        raise RuntimeError('Isolated sshd failed: ' + log.read())
                    try:
                        with socket.create_connection(('127.0.0.1', port), timeout=.1):
                            break
                    except OSError:
                        time.sleep(.1)
                env = os.environ.copy()
                env.update(DGL_TEST_PORT=str(port), DGL_TEST_USER=getpass.getuser(), DGL_TEST_KEY_FILE=str(root / 'client'),
                           DGL_TEST_RSA_FILE=str(root / 'rsa'), DGL_TEST_ENCRYPTED_FILE=str(root / 'encrypted'),
                           DGL_TEST_PASSPHRASE=passphrase, DGL_TEST_FINGERPRINT=fingerprint,
                           DGL_TEST_TMUX_VERSION=run([tmux, '-V']).stdout.strip().removeprefix('tmux '))
                command = args.command or ['./gradlew', ':app:desktopTest', '--rerun-tasks']
                print('Isolated OpenSSH ready; ' + run([tmux, '-V']).stdout.strip(), flush=True)
                result = subprocess.run(command, env=env)
                if result.returncode:
                    raise SystemExit(result.returncode)
            finally:
                subprocess.run([tmux, '-S', str(root / 'tmux.socket'), 'kill-server'], capture_output=True)
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()


if __name__ == '__main__':
    main()
