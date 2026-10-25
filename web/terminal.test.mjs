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

test('composer captures the renderer paste once, blocks background input and enforces encoded size', () => {
  const messages = [], listeners = {};
  let bracketed = false, focused = 0;
  class Terminal {
    parser = { registerOscHandler() {} };
    loadAddon() {} open() {} onResize() {}
    focus() { focused++; }
    onData(callback) { listeners.data = callback; }
    onBinary(callback) { listeners.binary = callback; }
    paste(text) {
      // xterm's current paste mode, independently changed between requests.
      const normalized = text.replace(/\r?\n/g, '\r');
      listeners.data(bracketed ? '\x1b[200~' + normalized + '\x1b[201~' : normalized);
    }
  }
  const window = { dgl: { postMessage: message => messages.push(JSON.parse(message)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} }, ResizeObserver: class { observe() {} },
    document: { getElementById: () => ({ addEventListener() {} }) },
  });
  messages.length = 0;
  window.setInputEnabled(false);
  listeners.data('background'); listeners.binary('\x1b[M');
  assert.equal(window.preparePaste('한글😀\n둘', false), '한글😀\r둘');
  bracketed = true;
  assert.equal(window.preparePaste('한글😀\n둘', true), '\x1b[200~한글😀\r둘\x1b[201~\r');
  assert.deepEqual(messages, []);
  assert.throws(() => window.preparePaste('', false));
  assert.throws(() => window.preparePaste('한'.repeat(21846), false));
  assert.equal(window.preparePaste('a'.repeat(65524), false).length, 65536);
  assert.throws(() => window.preparePaste('a'.repeat(65524), true));
  window.setInputEnabled(true); listeners.data('after');
  assert.deepEqual(messages, [{ type: 'input', data: 'after' }]);
  window.focusTerminal(); window.focusTerminal();
  assert.equal(focused, 2);
});

function cachedRenderer() {
  const messages = [], frames = [], timers = new Map(), animations = [];
  let render, nextTimer = 0;
  class Element {
    children = []; classes = new Set(); textContent = '';
    style = {}; width = 100; height = 100;
    classList = { add: value => this.classes.add(value), remove: value => this.classes.delete(value) };
    setAttribute(name, value) { this[name] = value; }
    appendChild(child) { this.children.push(child); child.parent = this; }
    remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
    cloneNode() { const copy = new Element(); copy.textContent = this.textContent; copy.width = this.width; copy.height = this.height; return copy; }
    get firstElementChild() { return this.children[0]; }
    getBoundingClientRect() { return { width: this.width, height: this.height }; }
    animate(keyframes, options) {
      const animation = { keyframes, options, cancelled: false, cancel() { this.cancelled = true; } };
      animations.push(animation); return animation;
    }
  }
  const root = new Element(), screen = new Element();
  screen.textContent = 'previous screen';
  root.querySelector = () => screen;
  class Terminal {
    element = root; rows = 24; parser = { registerOscHandler() {} };
    loadAddon() {} open() {} onData() {} onBinary() {} onResize() {} refresh() {}
    onRender(callback) { render = callback; return { dispose() { render = null; } }; }
  }
  const window = { dgl: { postMessage: message => messages.push(JSON.parse(message)) }, matchMedia: () => ({ matches: false }) };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder,
    FitAddon: { FitAddon: class {} }, ResizeObserver: class { observe() {} },
    document: { getElementById: () => ({ addEventListener() {} }), createElement: () => new Element() },
    requestAnimationFrame: callback => frames.push(callback),
    setTimeout: (callback, delay) => { timers.set(++nextTimer, { callback, delay }); return nextTimer; },
    clearTimeout: id => timers.delete(id),
  });
  messages.length = 0;
  return { window, root, screen, messages, animations,
    frame() { frames.splice(0).forEach(callback => callback()); },
    render() { render?.(); },
    expire(delay) { for (const [id, timer] of [...timers]) if (timer.delay === delay) { timers.delete(id); timer.callback(); } },
  };
}

test('cached viewport stays opaque until a rendered replacement, then fades out', () => {
  const r = cachedRenderer();
  r.window.beginSnapshot(); r.frame();
  const cache = r.root.children[0];
  assert.equal(cache['aria-hidden'], 'true');
  assert.equal(cache.children[0].textContent, 'previous screen');
  assert.ok(cache.classes.has('is-updating'));
  r.screen.textContent = 'replacement';
  r.window.endSnapshot(7);
  assert.deepEqual(r.messages, []);
  assert.ok(!cache.classes.has('is-releasing'));
  r.render(); r.frame();
  assert.deepEqual(r.messages, []);
  r.frame();
  assert.deepEqual(r.messages, [{ type: 'ack', id: 7 }]);
  assert.ok(cache.classes.has('is-releasing'));
  assert.equal(r.root.children.length, 1);
  r.expire(180);
  assert.equal(r.root.children.length, 0);
  assert.equal(r.animations.length, 0, 'advanced motion is opt-in');
});

test('opt-in transition morphs differing sizes with crossfade and blur', () => {
  const r = cachedRenderer();
  r.window.setSmoothTransitions(true);
  r.window.beginSnapshot(); r.frame();
  r.screen.width = 200; r.screen.height = 50;
  r.window.endSnapshot(10); r.render(); r.frame(); r.frame();
  assert.equal(r.animations.length, 2);
  assert.equal(r.animations[0].keyframes[1].transform, 'scale(2,0.5)');
  assert.equal(r.animations[1].keyframes[0].transform, 'scale(0.5,2)');
  assert.equal(r.animations[1].keyframes[0].opacity, 0);
  assert.equal(r.animations[1].keyframes[0].filter, 'blur(2px)');
  assert.equal(r.animations[1].keyframes[1].filter, 'blur(0px)');
  assert.equal(r.animations[0].options.duration, 240);
  r.expire(240);
  assert.equal(r.root.children.length, 0);
  assert.ok(r.animations.every(animation => animation.cancelled));
});

test('reduced motion and disabling the setting cancel enhanced motion', () => {
  const r = cachedRenderer();
  r.window.setSmoothTransitions(true);
  r.window.beginSnapshot(); r.frame();
  r.window.endSnapshot(1); r.render(); r.frame(); r.frame();
  r.window.setSmoothTransitions(false);
  assert.ok(r.animations.every(animation => animation.cancelled));
  r.window.cancelSnapshot();
  r.animations.length = 0;
  r.window.matchMedia = () => ({ matches: true });
  r.window.setSmoothTransitions(true);
  r.window.beginSnapshot(); r.frame();
  r.window.endSnapshot(2); r.render(); r.frame(); r.frame();
  assert.equal(r.animations.length, 0);
  r.expire(0);
  assert.equal(r.root.children.length, 0);
});

test('consecutive snapshots reuse one cache and stale fade callbacks cannot remove it', () => {
  const r = cachedRenderer();
  r.window.beginSnapshot();
  const cache = r.root.children[0];
  r.window.endSnapshot(1); r.render(); r.frame(); r.frame();
  r.window.beginSnapshot(); r.frame();
  r.expire(180);
  assert.deepEqual(r.root.children, [cache]);
  assert.ok(!cache.classes.has('is-releasing'));
  r.window.cancelSnapshot(); r.frame();
  assert.equal(r.root.children.length, 0);
});

test('cancellation and a stalled renderer release the cache and pending ack', () => {
  for (const cancel of [r => r.window.cancelSnapshot(), r => r.expire(15000)]) {
    const r = cachedRenderer();
    r.window.beginSnapshot(); r.frame();
    r.window.endSnapshot(9);
    cancel(r); r.render(); r.frame(); r.frame();
    assert.equal(r.root.children.length, 0);
    assert.deepEqual(r.messages, [{ type: 'ack', id: 9 }]);
  }
});
