#!/usr/bin/env python3
"""Disposable pane workload; constant-size progress file, no retained terminal output."""
import json
import os
from pathlib import Path
import select
import sys
import time
import tty


def main():
    root = Path(sys.argv[1])
    tty.setraw(sys.stdin.fileno())
    sequence = total = received = 0
    deadline = time.monotonic()
    while True:
        if select.select([sys.stdin], [], [], 0)[0]:
            received += len(os.read(sys.stdin.fileno(), 65536))
        if not (root / 'paused').exists():
            sequence += 1
            line = f'\x1b[32mDGL:{sequence:09d} 한글 日本語\x1b[0m abcdefghijklmnopqrstuvwxyz\r\n'.encode()
            block = (line * (16384 // len(line) + 1))[:16384]
            # Keep valid UTF-8/ANSI line boundaries, approximately 64 KiB/s.
            block = line * (16384 // len(line))
            offset = 0
            while offset < len(block):
                offset += os.write(sys.stdout.fileno(), block[offset:])
            total += len(block)
        temp = root / 'progress.tmp'
        temp.write_text(json.dumps(dict(pid=os.getpid(), sequence=sequence, bytes=total, inputBytes=received)))
        temp.replace(root / 'progress.json')
        deadline += .25
        time.sleep(max(0, deadline - time.monotonic()))
        # Do not burst/catch up after actual backpressure.
        deadline = max(deadline, time.monotonic() - .25)


if __name__ == '__main__':
    main()
