import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

test('client viewport uses container pixels and ignores zero layouts and pane grid changes', () => {
  const messages = []; let observe, render;
  const root = { clientWidth: 1000, clientHeight: 600, addEventListener() {} };
  let terminal;
  class Terminal {
    cols = 80; rows = 24; options = {}; parser = { registerOscHandler() {} };
    element = { querySelector: () => ({ getBoundingClientRect: () => ({ width: this.cols * 10, height: this.rows * 20 }) }) };
    constructor() { terminal = this; }
    loadAddon() {} open() {} onData() {} onBinary() {}
    onRender(callback) { render = callback; }
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class { fit() { throw new Error('No pane fit'); } } },
    document: { getElementById: () => root },
    ResizeObserver: class { constructor(callback) { observe = callback; } observe() { observe(); } },
  });
  assert.deepEqual(messages.splice(0), [{ type: 'resize', columns: 100, rows: 30 }, { type: 'ready' }]);
  root.clientHeight = 0; observe();
  assert.deepEqual(messages, [], 'hidden/recreated WebViews must not shrink tmux to two rows');
  root.clientHeight = 600;
  window.resizePane(40, 12); render();
  assert.equal(terminal.rows, 12);
  assert.deepEqual(messages, [], 'server grid changes cannot request a client resize');
  root.clientHeight = 240; observe();
  assert.deepEqual(messages, [{ type: 'resize', columns: 100, rows: 12 }]);
});

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
    assert.deepEqual(messages.splice(0), [{ type: 'input', data, pane: 'shell', target: null }]);
  }
  const data = '\x1b[M \x85\xff';
  listeners.binary(data);
  assert.deepEqual(messages.splice(0), [{ type: 'input-binary', data, pane: 'shell', target: null }]);
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
  assert.deepEqual(messages, [{ type: 'input', data: 'after', pane: 'shell', target: null }]);
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
  r.expire(90);
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
  assert.equal(r.animations[0].options.duration, 120);
  r.expire(120);
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
  r.expire(90);
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

function imeRenderer() {
  const messages = [], frames = [];
  const root = { clientWidth: 1000, clientHeight: 600, addEventListener() {} };
  const canvas = { style: {} }, host = { addEventListener() {} };
  let observe, render, terminal;
  class Terminal {
    cols = 100; rows = 30; options = {}; parser = { registerOscHandler() {} };
    buffer = { active: { viewportY: 10, baseY: 10 } };
    element = { querySelector: () => ({ getBoundingClientRect: () => ({ width: this.cols * 10, height: this.rows * 20 }) }) };
    constructor() { terminal = this; }
    loadAddon() {} open() {} onData() {} onBinary() {}
    onRender(callback) { render = callback; }
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} },
    document: { getElementById: id => id === 'terminal' ? root : id === 'window' ? canvas : host },
    ResizeObserver: class { constructor(callback) { observe = callback; } observe() { observe(); } },
    requestAnimationFrame: callback => frames.push(callback),
  });
  messages.length = 0;
  return { window, root, canvas, messages, terminal, observe: () => observe(), render: () => render(),
    frame() { frames.splice(0).forEach(callback => callback()); } };
}

test('IME show and hide anchor the existing surface and submit only the final grid', () => {
  const r = imeRenderer();
  r.window.setImeAnimation(true, 1);
  for (const height of [540, 480, 420, 360]) { r.root.clientHeight = height; r.observe(); }
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 1); r.frame();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 18 }]);
  r.window.resizePane(100, 18); r.render();
  assert.equal(r.canvas.style.transform, 'translateY(0px)');
  r.window.setImeAnimation(true, 2);
  for (const height of [420, 480, 540, 600]) { r.root.clientHeight = height; r.observe(); }
  assert.equal(r.canvas.style.transform, 'translateY(240px)');
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 2); r.frame();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 30 }]);
});

test('cancelled and rapidly reversed IME animations ignore old endings and settle latest geometry', () => {
  const r = imeRenderer();
  r.window.setImeAnimation(true, 1);
  r.root.clientHeight = 400; r.observe();
  r.window.setImeAnimation(false, 1);
  r.window.setImeAnimation(true, 2);
  r.frame();
  assert.deepEqual(r.messages, []);
  r.root.clientHeight = 600; r.observe();
  r.window.setImeAnimation(false, 1); r.frame();
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 2); r.frame();
  assert.equal(r.canvas.style.transform, '');
  assert.deepEqual(r.messages, [], 'a cancelled show returns to the existing grid');
  r.window.setImeAnimation(true, 3);
  r.root.clientHeight = 400; r.observe();
  r.window.setImeAnimation(false, 3); r.frame();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 20 }]);
});

test('IME preserves an existing history reading position', () => {
  const r = imeRenderer();
  r.terminal.buffer.active.viewportY = 3;
  r.window.setImeAnimation(true, 1);
  r.root.clientHeight = 300; r.observe();
  assert.equal(r.canvas.style.transform, '');
  assert.equal(r.terminal.buffer.active.viewportY, 3);
  r.window.setImeAnimation(false, 1); r.frame();
  assert.equal(r.canvas.style.transform, '');
  assert.equal(r.terminal.buffer.active.viewportY, 3);
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 15 }]);
});

test('a replacement snapshot releases its predecessor acknowledgement and reduced durations are bundled', () => {
  const r = cachedRenderer();
  r.window.beginSnapshot(); r.window.endSnapshot(11);
  r.window.beginSnapshot(); r.frame();
  assert.deepEqual(r.messages, [{ type: 'ack', id: 11 }]);
  assert.equal(r.root.children.length, 1);
  r.window.cancelSnapshot();
  assert.equal(r.root.children.length, 0);
  const css = readFileSync(new URL('../app/src/androidMain/assets/terminal/style.css', import.meta.url), 'utf8');
  assert.ok(css.includes('opacity 90ms'));
  assert.ok(css.includes('opacity 80ms'));
});
