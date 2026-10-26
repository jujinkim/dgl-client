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
    removeAttribute(name) { delete this[name]; }
    appendChild(child) { this.children.push(child); child.parent = this; }
    remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
    cloneNode(deep) {
      const copy = new Element(); copy.textContent = this.textContent; copy.width = this.width; copy.height = this.height;
      copy.id = this.id; copy.tagName = this.tagName;
      if (deep) this.children.forEach(child => copy.appendChild(child.cloneNode(true)));
      return copy;
    }
    querySelectorAll(selector) {
      return this.children.flatMap(child => [
        ...(selector === '[id]' ? child.id ? [child] : [] : child.tagName === 'CANVAS' ? [child] : []),
        ...child.querySelectorAll(selector),
      ]);
    }
    getContext() { return { drawImage: original => { this.pixels = original.pixels; } }; }
    get firstElementChild() { return this.children[0]; }
    getBoundingClientRect() { return { width: this.width, height: this.height }; }
    animate(keyframes, options) {
      const animation = { keyframes, options, cancelled: false, cancel() { this.cancelled = true; } };
      animations.push(animation); return animation;
    }
  }
  const root = new Element(), screen = new Element(), canvas = new Element(), body = new Element();
  canvas.id = 'window'; canvas.appendChild(screen);
  const viewport = { clientWidth: 0, clientHeight: 0, scrollLeft: 13, scrollTop: 29, addEventListener() {} };
  screen.textContent = 'previous screen';
  root.querySelector = () => screen;
  class Terminal {
    element = root; rows = 24; parser = { registerOscHandler() {} };
    loadAddon() {} open() {} onData() {} onBinary() {} onResize() {} refresh() {}
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
    onRender(callback) { render = callback; return { dispose() { render = null; } }; }
  }
  const window = { dgl: { postMessage: message => messages.push(JSON.parse(message)) }, matchMedia: () => ({ matches: false }) };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder,
    FitAddon: { FitAddon: class {} }, ResizeObserver: class { observe() {} },
    document: { body, getElementById: id => id === 'window' ? canvas : viewport, createElement: () => new Element() },
    requestAnimationFrame: callback => frames.push(callback),
    setTimeout: (callback, delay) => { timers.set(++nextTimer, { callback, delay }); return nextTimer; },
    clearTimeout: id => timers.delete(id),
  });
  messages.length = 0;
  return { window, root, screen, canvas, body, Element, messages, animations,
    frame() { frames.splice(0).forEach(callback => callback()); },
    render() { render?.(); },
    expire(delay) { for (const [id, timer] of [...timers]) if (timer.delay === delay) { timers.delete(id); timer.callback(); } },
  };
}

test('reconnecting viewport stays opaque until a rendered replacement, then fades out', () => {
  const r = cachedRenderer();
  r.window.setConnectionRecovery(true);
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

test('ordinary resize snapshots swap after rendering without dim or motion regardless of the setting', () => {
  for (const smooth of [false, true]) {
    const r = cachedRenderer();
    r.window.setSmoothTransitions(smooth);
    r.window.beginSnapshot(); r.frame();
    const cache = r.root.children[0];
    assert.equal(cache.children[0].textContent, 'previous screen');
    assert.ok(!cache.classes.has('is-recovering'));
    assert.ok(!cache.classes.has('is-updating'));
    r.window.resizePane(160, 12);
    r.screen.textContent = 'resized screen';
    r.screen.width = 200; r.screen.height = 50;
    r.window.endSnapshot(8);
    assert.deepEqual(r.messages, []);
    r.render(); r.frame();
    assert.equal(r.root.children.length, 1, 'hold the previous viewport until a paint opportunity');
    assert.deepEqual(r.messages, []);
    r.frame();
    assert.equal(r.root.children.length, 0, 'no fade timer delays an ordinary resize');
    assert.deepEqual(r.messages, [{ type: 'ack', id: 8 }]);
    assert.ok(!cache.classes.has('is-releasing'));
    assert.equal(r.animations.length, 0);
  }
});

test('a resize superseding a reconnect fade removes recovery motion and keeps its render acknowledgement', () => {
  const r = cachedRenderer();
  r.window.setConnectionRecovery(true);
  r.window.setSmoothTransitions(true);
  r.window.beginSnapshot(); r.frame();
  r.window.endSnapshot(1); r.render(); r.frame(); r.frame();
  const cache = r.root.children[0];
  assert.ok(cache.classes.has('is-releasing'));
  r.window.setConnectionRecovery(false);
  r.window.beginSnapshot(); r.window.resizePane(160, 12); r.frame();
  assert.deepEqual(r.root.children, [cache]);
  assert.ok(!cache.classes.has('is-recovering'));
  assert.ok(!cache.classes.has('is-updating'));
  assert.ok(!cache.classes.has('is-releasing'));
  assert.equal(cache.style.transitionDuration, '0ms');
  assert.ok(r.animations.every(animation => animation.cancelled));
  r.animations.length = 0;
  r.expire(120);
  assert.deepEqual(r.root.children, [cache]);
  r.window.endSnapshot(2); r.render(); r.frame(); r.frame();
  assert.equal(r.root.children.length, 0);
  assert.equal(r.animations.length, 0);
  assert.deepEqual(r.messages, [{ type: 'ack', id: 1 }, { type: 'ack', id: 2 }]);
});

test('connection recovery keeps the previous rendered window through layout replacement and failed snapshots', () => {
  const r = cachedRenderer();
  const bitmap = new r.Element(); bitmap.tagName = 'CANVAS'; bitmap.pixels = 'rendered pixels';
  r.canvas.appendChild(bitmap);
  r.window.setConnectionRecovery(true);
  const cache = r.body.children[0], previous = cache.firstElementChild;
  assert.equal(cache.className, 'connection-cache');
  assert.equal(cache['aria-hidden'], 'true');
  assert.equal(cache.inert, true);
  assert.equal(previous.id, undefined, 'frozen markup must not replace live element identities');
  assert.equal(previous.style.left, '-13px'); assert.equal(previous.style.top, '-29px');
  assert.equal(previous.children[0].textContent, 'previous screen');
  assert.equal(previous.children[1].pixels, 'rendered pixels', 'canvas pixels need an explicit copy');
  r.screen.textContent = 'partial replacement';
  r.canvas.children = [];
  r.window.beginSnapshot(); r.window.cancelSnapshot(); r.expire(15000);
  r.window.setConnectionRecovery(true);
  assert.equal(r.body.children.length, 1, 'retries reuse one frozen viewport');
  assert.equal(r.body.children[0], cache);
  assert.equal(previous.children[0].textContent, 'previous screen');
  r.window.setConnectionRecovery(false);
  assert.equal(r.body.children.length, 0, 'only recovery completion releases the frozen window');
  r.window.setConnectionRecovery(false);
});

test('opt-in reconnect transition morphs differing sizes with crossfade and blur', () => {
  const r = cachedRenderer();
  r.window.setConnectionRecovery(true);
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
  r.window.setConnectionRecovery(true);
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
  assert.equal(r.root.children.length, 0);
});

test('consecutive snapshots reuse one cache and stale fade callbacks cannot remove it', () => {
  const r = cachedRenderer();
  r.window.setConnectionRecovery(true);
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
  const messages = [], frames = [], terminals = [];
  const root = { clientWidth: 1000, clientHeight: 600, addEventListener() {} };
  const host = () => ({ style: {}, addEventListener() {}, remove() {}, classList: { toggle() {} } });
  const canvas = { style: {}, appendChild() {} };
  let observe, render, terminal;
  class Terminal {
    cols = 100; rows = 30; options = {}; parser = { registerOscHandler() {} };
    buffer = { active: { viewportY: 10, baseY: 10 } };
    element = { querySelector: () => ({ getBoundingClientRect: () => ({ width: this.cols * 10 * (this.options.fontSize || 14) / 14, height: this.rows * 20 * (this.options.fontSize || 14) / 14 }) }) };
    constructor() { terminal ||= this; terminals.push(this); }
    loadAddon() {} open() {} dispose() {}
    onData(callback) { this.data = callback; } onBinary(callback) { this.binary = callback; }
    onRender(callback) { render = callback; }
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} },
    document: { getElementById: id => id === 'terminal' ? root : id === 'window' ? canvas : host(), createElement: host, hasFocus: () => false },
    ResizeObserver: class { constructor(callback) { observe = callback; } observe() { observe(); } },
    requestAnimationFrame: callback => frames.push(callback),
  });
  messages.length = 0;
  return { window, root, canvas, messages, terminal, terminals, observe: () => observe(), render: () => render(),
    frame() { frames.splice(0).forEach(callback => callback()); } };
}

test('outer scrolling follows the remote window, not the hidden measurement shell', () => {
  const r = imeRenderer();
  const layout = (columns, rows = 24) => ({ window: '@1', columns, rows, panes: [], active: null });
  r.root.scrollLeft = 1500; r.root.scrollTop = 700;
  r.window.setLayout(layout(40));
  assert.equal(r.canvas.style.width, '1000px'); assert.equal(r.root.scrollLeft, 0);
  assert.equal(r.canvas.style.height, '600px'); assert.equal(r.root.scrollTop, 0);
  r.window.resizePane(500, 200);
  assert.equal(r.canvas.style.width, '1000px', 'a hidden shell with 500 columns must not create blank outer scrolling');
  assert.equal(r.canvas.style.height, '600px');
  assert.deepEqual(r.messages, [], 'server geometry must not feed back into the client resize');
  r.window.setLayout(layout(200, 60));
  assert.equal(r.canvas.style.width, '2000px'); assert.equal(r.canvas.style.height, '1200px');
  r.root.scrollLeft = 1000; r.root.scrollTop = 600;
  r.root.clientWidth = 1600; r.root.clientHeight = 1000; r.observe();
  assert.equal(r.root.scrollLeft, 400); assert.equal(r.root.scrollTop, 200);
  r.window.setLayout(layout(20));
  assert.equal(r.root.scrollLeft, 0); assert.equal(r.root.scrollTop, 0);
  r.window.setLayout({ window: null, panes: [] });
  r.window.resizePane(200, 60);
  assert.equal(r.canvas.style.width, '2000px', 'ordinary SSH keeps access to its real wider grid');
  r.window.resizePane(40, 12);
  assert.equal(r.canvas.style.width, '1600px'); assert.equal(r.canvas.style.height, '1000px');
});

test('pinch updates every pane locally, suppresses input and submits only the final viewport', () => {
  const r = imeRenderer();
  r.window.setLayout({ window: '@1', columns: 100, rows: 30, active: '%1', inputTarget: 'current',
    panes: [{ id: '%1', visible: true, left: 0, top: 0, width: 100, height: 30 }] });
  r.window.setFontGesture(true);
  r.window.configure(16, 2000); r.render();
  r.window.configure(18, 3000); r.render();
  assert.deepEqual(r.terminals.map(t => t.options.fontSize), [18, 18]);
  assert.equal(r.canvas.style.width, `${100 * 10 * 18 / 14}px`);
  r.terminals[1].data('accidental input'); r.terminals[1].binary('\x1b[M');
  assert.deepEqual(r.messages, [], 'neither remote resize nor terminal input escapes an active pinch');
  r.window.outputPane('%2', 'new');
  assert.equal(r.terminals[2].options.fontSize, 18, 'new panes inherit the session font');
  r.window.setFontGesture(false); r.render(); r.window.setFontGesture(false);
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 77, rows: 23 }]);
  r.terminals[1].data('after');
  assert.deepEqual(r.messages.splice(0), [{ type: 'input', data: 'after', pane: '%1', target: 'current' }]);
  r.window.configure(14, 3000); r.render();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 30 }]);
});

test('IME and pinch only submit a final viewport after both gestures have ended', () => {
  const r = imeRenderer();
  r.window.setFontGesture(true); r.window.configure(28, 5000);
  r.window.setImeAnimation(true, 1); r.root.clientHeight = 400; r.observe();
  r.window.setFontGesture(false); r.render();
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 1); r.frame(); r.render();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 50, rows: 10 }]);
});

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

function keyboardRenderer() {
  const messages = [], listeners = {}, events = {}, timers = [];
  let terminal;
  class Terminal {
    modes = { applicationCursorKeysMode: false }; parser = { registerOscHandler() {} };
    constructor() { terminal = this; }
    loadAddon() {} open() {} onResize() {}
    onKey(callback) { listeners.key = callback; }
    attachCustomKeyEventHandler(callback) { listeners.custom = callback; }
    onData(callback) { listeners.data = callback; }
    onBinary(callback) { listeners.binary = callback; }
    paste(text) { listeners.data(text); }
  }
  const host = { style: {}, addEventListener: (name, callback) => { events[name] = callback; } };
  const window = { dgl: { postMessage: message => messages.push(JSON.parse(message)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} }, ResizeObserver: class { observe() {} },
    document: { getElementById: () => host, hasFocus: () => false }, setTimeout: callback => timers.push(callback),
  });
  messages.length = 0;
  return { window, terminal, messages, listeners, events, timers,
    key(key, data, modifiers = {}) {
      const domEvent = { type: 'keydown', key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...modifiers };
      listeners.custom(domEvent); listeners.key({ domEvent }); listeners.data(data);
    },
  };
}

test('semantic keys use the shared encoder channel once and preserve cursor mode and physical modifiers', () => {
  const r = keyboardRenderer();
  r.terminal.modes.applicationCursorKeysMode = true;
  r.key('ArrowLeft', '\x1b[1;7D', { ctrlKey: true, altKey: true });
  r.key('c', 'c');
  r.window.sendShortcut('left', false, true, false);
  assert.deepEqual(r.messages, [
    { type: 'keyboard', key: 'left', text: null, control: true, shift: false, alt: true, applicationCursor: true, pane: 'shell', target: null },
    { type: 'keyboard', key: 'c', text: 'c', control: false, shift: false, alt: false, applicationCursor: true, pane: 'shell', target: null },
    { type: 'keyboard', key: 'left', text: null, control: false, shift: true, alt: false, applicationCursor: true, pane: 'shell', target: null },
  ]);
  r.messages.length = 0;
  r.window.setInputEnabled(false); r.key('c', 'c'); r.window.sendShortcut('left', false, false, false);
  assert.deepEqual(r.messages, []);
});

test('IME commits once while paste mouse and protocol responses bypass semantic input', () => {
  const r = keyboardRenderer();
  r.events.compositionstart();
  r.events.input({ inputType: 'insertCompositionText', data: 'ㅎ' });
  r.events.input({ inputType: 'insertCompositionText', data: '한' });
  assert.deepEqual(r.messages, [], 'intermediate composition events do not submit anything');
  r.listeners.data('\x1b[0n'); // a terminal protocol response during composition
  r.events.compositionend();
  r.listeners.data('한😀'); // finalized delta from xterm, not compositionend.data
  r.events.paste(); r.listeners.data('pasted text');
  r.timers.splice(0).forEach(callback => callback());
  r.listeners.data('\x1b[<0;1;1M'); r.listeners.binary('\x1b[M \x85\xff');
  assert.equal(r.window.preparePaste('composer', false), 'composer');
  assert.deepEqual(r.messages, [
    { type: 'input', data: '\x1b[0n', pane: 'shell', target: null },
    { type: 'keyboard', key: 'text', text: '한😀', control: false, shift: false, alt: false, applicationCursor: false, pane: 'shell', target: null },
    { type: 'input', data: 'pasted text', pane: 'shell', target: null },
    { type: 'input', data: '\x1b[<0;1;1M', pane: 'shell', target: null },
    { type: 'input-binary', data: '\x1b[M \x85\xff', pane: 'shell', target: null },
  ]);
});


test('delayed IME commit retains the original target across pane or reconnect changes', () => {
  const r = keyboardRenderer();
  r.window.setLayout({ window: null, active: 'shell', inputTarget: 'generation-one', panes: [] });
  r.events.compositionstart();
  r.events.input({ inputType: 'insertCompositionText', data: '한' });
  r.window.setLayout({ window: null, active: 'shell', inputTarget: 'generation-two', panes: [] });
  r.events.compositionend(); r.listeners.data('한');
  assert.equal(r.messages.length, 1);
  assert.equal(r.messages[0].target, 'generation-one', 'native validation must reject an old commit instead of retargeting it');
  r.key('c', 'c');
  assert.equal(r.messages[1].target, 'generation-two');
  r.events.input({ inputType: 'insertText', data: 'null' }); r.listeners.data('null');
  assert.equal(r.messages[2].text, 'null');
  assert.equal(r.messages[2].target, 'generation-two');
  // An empty/canceled composition must not keep an old destination for later input.
  r.events.compositionstart(); r.events.compositionend();
  r.timers.splice(0).forEach(callback => callback());
  r.window.setLayout({ window: null, active: 'shell', inputTarget: 'generation-three', panes: [] });
  r.events.input({ inputType: 'insertText', data: 'next' }); r.listeners.data('next');
  assert.equal(r.messages[3].target, 'generation-three');
});
