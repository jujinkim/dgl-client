import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

test('terminal bridge forwards text and raw mouse input through distinct channels', () => {
  const messages = [];
  const listeners = {};
  class Terminal {
    parser = { registerOscHandler() {} };
    loadAddon() {}
    open() {}
    onData(callback) { listeners.data = callback; }
    onBinary(callback) { listeners.binary = callback; }
    onResize() {}
  }
  const context = {
    Terminal, TextEncoder,
    FitAddon: { FitAddon: class {} },
    ResizeObserver: class { observe() {} },
    document: { getElementById: () => ({ addEventListener() {} }) },
    window: { dgl: { postMessage: message => messages.push(JSON.parse(message)) } },
  };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), context);
  assert.deepEqual(messages.splice(0), [{ type: 'ready' }]);
  for (const data of ['한글😀', '\x1b[1;6D', '\x1b[<35;1;1M', '\x1b[<64;1;1M']) {
    listeners.data(data);
    assert.deepEqual(messages.splice(0), [{ type: 'input', data }]);
  }
  const data = '\x1b[M \x85\xff';
  listeners.binary(data);
  assert.deepEqual(messages.splice(0), [{ type: 'input-binary', data }]);
  assert.deepEqual([...messages], []);
  listeners.binary('a'.repeat(65537));
  listeners.data('한'.repeat(21846));
  assert.deepEqual(messages, []);
});
