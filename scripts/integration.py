#!/usr/bin/env python3
"""Run SSH/tmux integration tests against a disposable OpenSSH + private tmux socket.

The default command is the Rust engine's ignored `ssh_integration` suite; the agent
observer case in it requires `--agent-home`. `--command` selects JVM reference or
Android runs instead.

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
import signal
import sys


def run(args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, encoding="utf-8", **kwargs)


def process_snapshot():
    rows = run(['ps', '-A', '-o', 'pid=', '-o', 'ppid=', '-o', 'args=']).stdout.splitlines()
    return {int(parts[0]): (int(parts[1]), parts[2]) for row in rows if len(parts := row.strip().split(None, 2)) == 3}


def fixture_processes(root, snapshot, server_pid=None):
    """Only exact private paths (or the PID returned by this private socket)."""
    owned = set()
    for pid, (_, command) in snapshot.items():
        try:
            args = shlex.split(command)
        except ValueError:
            args = command.split()
        if any(arg.startswith(str(root) + '/') for arg in args):
            owned.add(pid)
    if server_pid in snapshot:
        owned.add(server_pid)
    while True:
        children = {pid for pid, (parent, _) in snapshot.items() if parent in owned}
        if children <= owned:
            return {pid: snapshot[pid] for pid in owned}
        owned |= children


def cleanup_tmux(root, tmux):
    server_pid = None
    try:
        server_pid = int(run([tmux, '-S', str(root / 'tmux.socket'), 'display-message', '-p', '#{pid}'], timeout=3).stdout)
    except (subprocess.SubprocessError, ValueError):
        pass
    owned = fixture_processes(root, process_snapshot(), server_pid)
    try:
        subprocess.run([tmux, '-S', str(root / 'tmux.socket'), 'kill-server'], capture_output=True, timeout=5)
    finally:
        # Older tmux can retain a dead control client while flushing its output.
        # kill-server returning is therefore not proof that its processes exited.
        for sig in (signal.SIGTERM, signal.SIGKILL):
            current = process_snapshot()
            for pid, identity in owned.items():
                if current.get(pid) == identity:
                    try:
                        os.kill(pid, sig)
                    except ProcessLookupError:
                        pass
            if sig == signal.SIGTERM:
                time.sleep(.2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--tmux', default=shutil.which('tmux'))
    parser.add_argument('--agent-home', action='store_true', help='Use a private HOME for agent observer integration tests')
    parser.add_argument('--tmux-login-path', action='store_true', help='Expose tmux only through an isolated login shell PATH')
    parser.add_argument('--command', nargs=argparse.REMAINDER)
    # argparse treats a literal `--` as its own terminator even after REMAINDER.
    # Preserve the child command verbatim (cargo/libtest needs that separator).
    argv = sys.argv[1:]
    split = argv.index('--command') if '--command' in argv else len(argv)
    args = parser.parse_args(argv[:split])
    args.command = argv[split + 1:] if split < len(argv) else None
    if args.command == []:
        parser.error('--command requires a child command')
    if not args.tmux:
        raise SystemExit('tmux is required')
    tmux = str(Path(args.tmux).resolve())
    if not Path(tmux).is_file():
        raise SystemExit('Test tmux binary does not exist')
    with tempfile.TemporaryDirectory(prefix='dgl-integration-') as folder:
        root = Path(folder)
        os.chmod(root, 0o700)
        for name, keytype in [('host', 'ed25519'), ('client', 'ed25519'), ('rsa', 'rsa')]:
            run(['ssh-keygen', '-q', '-t', keytype, '-N', '', '-f', str(root / name)])
        passphrase = secrets.token_urlsafe(24)
        run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', passphrase, '-f', str(root / 'encrypted')])
        (root / 'authorized_keys').write_text(''.join((root / (name + '.pub')).read_text(encoding="utf-8") for name in ['client', 'rsa', 'encrypted']), encoding="utf-8")
        fingerprint = run(['ssh-keygen', '-lf', str(root / 'host.pub'), '-E', 'sha256']).stdout.split()[1]
        # A space and quote exercise executable-path quoting as well as discovery.
        binary_dir = root / ("login tools' bin" if args.tmux_login_path else 'bin')
        binary_dir.mkdir()
        normal_path = str(binary_dir) + ':/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin'
        (root / 'tmux.conf').write_text('set -g default-shell /bin/sh\nset -g default-command /bin/sh\nset -g automatic-rename off\nset-environment -g LANG en_US.UTF-8\nset-environment -g LC_ALL en_US.UTF-8\n', encoding="utf-8")
        wrapper = binary_dir / 'tmux'
        wrapper.write_text('#!/bin/sh\nexport PATH=' + shlex.quote(normal_path) + '\nexec ' + shlex.quote(tmux) + ' -S ' + shlex.quote(str(root / 'tmux.socket')) + ' -f ' + shlex.quote(str(root / 'tmux.conf')) + ' "$@"\n', encoding="utf-8")
        wrapper.chmod(0o700)
        environment = 'export PATH=' + shlex.quote(normal_path) + '\n'
        if args.agent_home:
            agent_home = root / 'home'
            agent_home.mkdir(mode=0o700)
            environment += 'export HOME=' + shlex.quote(str(agent_home)) + '\nexport CODEX_HOME=' + shlex.quote(str(agent_home / '.codex')) + '\nexport CLAUDE_CONFIG_DIR=' + shlex.quote(str(agent_home / '.claude')) + '\n'
        if args.tmux_login_path:
            login_shell = root / 'login-shell'
            login_shell.write_text('#!/bin/sh\n[ "$1" = "-lc" ] || exit 2\nprintf "Login profile banner\\n"\n' + environment + 'exec /bin/sh -c "$2"\n', encoding="utf-8")
            login_shell.chmod(0o700)
            (root / 'empty-path').mkdir()
            environment = 'export PATH=' + shlex.quote(str(root / 'empty-path')) + '\nexport SHELL=' + shlex.quote(str(login_shell)) + '\n'
        dispatcher = root / 'dispatch.sh'
        dispatcher.write_text('#!/bin/sh\nunset TMUX\n' + environment + 'if [ -n "$SSH_ORIGINAL_COMMAND" ]; then exec /bin/sh -c "$SSH_ORIGINAL_COMMAND"; else exec /bin/sh -i; fi\n', encoding="utf-8")
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
                env.update(DGL_TEST_ROOT=str(root), DGL_TEST_PORT=str(port), DGL_TEST_USER=getpass.getuser(), DGL_TEST_KEY_FILE=str(root / 'client'),
                           DGL_TEST_RSA_FILE=str(root / 'rsa'), DGL_TEST_ENCRYPTED_FILE=str(root / 'encrypted'),
                           DGL_TEST_PASSPHRASE=passphrase, DGL_TEST_FINGERPRINT=fingerprint,
                           DGL_TEST_TMUX_LOGIN_PATH='1' if args.tmux_login_path else '0',
                           DGL_TEST_TMUX_WRAPPER=str(wrapper),
                           DGL_TEST_TMUX_VERSION=run([tmux, '-V']).stdout.strip().removeprefix('tmux '))
                command = args.command or ['cargo', 'test', '--locked', '-p', 'dgl-core', '--test', 'ssh_integration',
                                           '--', '--ignored', '--test-threads=1']
                print('Isolated OpenSSH ready; ' + run([tmux, '-V']).stdout.strip(), flush=True)
                result = subprocess.Popen(command, env=env)
                try:
                    result.wait()
                finally:
                    if result.poll() is None:
                        result.terminate()
                        try:
                            result.wait(timeout=30)
                        except subprocess.TimeoutExpired:
                            result.kill(); result.wait()
                if result.returncode:
                    raise SystemExit(result.returncode)
            finally:
                try:
                    cleanup_tmux(root, tmux)
                finally:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill(); process.wait()


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    main()
