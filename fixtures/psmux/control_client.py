#!/usr/bin/env python3
# Deterministic psmux 3.4 control/CLI fixture, never invokes a real shell or multiplexer.
import json
import os
from pathlib import Path
import shlex
import sys
state_path = Path(sys.argv[1])
args = sys.argv[2:]
state = json.loads(state_path.read_text())
def log(args):
    with state_path.with_suffix('.log').open('a') as out:
        out.write(json.dumps({'pid': os.getpid(), 'args': args, 'session': os.environ.get('PSMUX_SESSION_NAME')}) + '\n')
log(args)
def target():
    return args[args.index('-t') + 1] if '-t' in args else '$1'
if args == ['-V']:
    print('tmux 3.4.0')
    print('psmux 3.4.0 (20aa1d7 2026-10-13)')
elif args == ['list-commands']:
    print('capture-pane refresh-client send-keys list-sessions')
elif args[0] == 'list-sessions':
    prefix = 's|' if args[args.index('-F') + 1].startswith('s|') else ''
    for session in state:
        print(f"{prefix}{session['id']}|{session['name']}|0")
elif args[0] == 'list-windows':
    name = next(session for session in state if session['id'] == target()).get('window', 'window')
    print(f'{target()}|@0|{name}|1|fixture-layout|0|80|24|0|fixture-layout')
elif args[0] == 'list-panes':
    print('%0|@0|80|24|1|||fixture|0|0')
elif args[0] == 'rename-session':
    for session in state:
        if session['id'] == target():
            session['name'] = args[-1]
    state_path.write_text(json.dumps(state))
elif args[0] == 'rename-window':
    session_id, window = target().split(':')
    assert window == '@0'
    for session in state:
        if session['id'] == session_id:
            session['window'] = args[-1]
    state_path.write_text(json.dumps(state))
elif args[0] == '-CC':
    selected = next(s for s in state if s['id'] == target())
    assert os.environ.get('PSMUX_SESSION_NAME') == selected['name']
    sys.stdout.write('\x1bP1000p%begin 1 1 0\n%end 1 1 0\n')
    sys.stdout.flush()
    number = 1
    for wire in sys.stdin:
        command = shlex.split(wire)
        log(command)
        if '-t' in command:
            assert command[command.index('-t') + 1] in ('%0', '@0'), command
        number += 1
        print(f'%begin 1 {number} 1')
        if command[0] == 'display-message':
            if 'cursor_x' in wire:
                print('0|0|0|1|0|0|0|0|0|0|0|0|0|23|0|80|24')
            else:
                print('fixture')
        elif command[0] == 'capture-pane':
            # Literal backslash and ANSI: psmux ignores tmux's -C capture escaping flag.
            print('fixture\\literal\x1b[0m')
        print(f'%end 1 {number} 1')
        if command[0] == 'capture-pane':
            print(r'%extended-output %0 0 : AFTER_CAPTURE\015\012')
        if command[:3] == ['refresh-client', '-f', 'pause-after=2']:
            print('%pause %0')
        sys.stdout.flush()
else:
    raise RuntimeError(args)
