import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

test('editor focus changes notify native keyboard ownership even while remote input is blocked', () => {
  const messages = [], listeners = {};
  class Terminal {
    parser = { registerOscHandler() {} };
    loadAddon() {} open() {} onData() {} onBinary() {}
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} }, ResizeObserver: class { observe() {} },
    document: { getElementById: () => ({ addEventListener: (name, callback) => { listeners[name] = callback; } }) },
  });
  messages.length = 0;
  window.setInputEnabled(false);
  listeners.focusin(); listeners.focusout();
  assert.deepEqual(messages.map(({ type, focused }) => ({ type, focused })), [
    { type: 'editor-focus', focused: true }, { type: 'editor-focus', focused: false },
  ]);
});

test('client viewport uses container pixels and ignores zero layouts and pane grid changes', () => {
  const messages = []; let observe, render;
  const root = { clientWidth: 1000, clientHeight: 600, addEventListener() {} };
  let terminal;
  class Terminal {
    cols = 80; rows = 24; options = {}; parser = { registerOscHandler() {} };
    element = { querySelector: () => ({ getBoundingClientRect: () => ({ width: this.cols * 10, height: this.rows * 20 }) }) };
    constructor() { terminal = this; }
    loadAddon(addon) { addon.term = this; } open() {} onData() {} onBinary() {}
    onRender(callback) { render = callback; }
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {
      fit() { this.term.resize(Math.floor((root.clientWidth - 14) / 10), Math.floor(root.clientHeight / 20)); }
    } },
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
  messages.length = 0;
  root.clientWidth = 807; window.resizePane(80, 12); render(); render();
  assert.equal(terminal.cols, 80, 'a render must not subtract the hidden 14px scrollbar and shrink 80 columns to 79');
  assert.deepEqual(messages, [{ type: 'resize', columns: 80, rows: 12 }]);
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
    assert.deepEqual(messages.splice(0), [{ type: 'input', data, ...(/^\x1b\[</.test(data) ? { pointer: true } : {}), pane: 'shell', target: null }]);
  }
  const data = '\x1b[M \x85\xff';
  listeners.binary(data);
  assert.deepEqual(messages.splice(0), [{ type: 'input-binary', data, pointer: true, pane: 'shell', target: null }]);
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
  assert.equal(window.preparePaste('한글😀\n둘'), '한글😀\r둘');
  bracketed = true;
  assert.equal(window.preparePaste('한글😀\n둘'), '\x1b[200~한글😀\r둘\x1b[201~');
  assert.deepEqual(messages.splice(0), [{ type: 'blocked-input' }]);
  assert.throws(() => window.preparePaste(''));
  assert.throws(() => window.preparePaste('한'.repeat(21846)));
  assert.equal(window.preparePaste('a'.repeat(65524)).length, 65536);
  assert.throws(() => window.preparePaste('a'.repeat(65525)));
  bracketed = false;
  assert.equal(window.preparePaste('a'.repeat(65536)).length, 65536);
  assert.throws(() => window.preparePaste('a'.repeat(65537)));
  assert.equal(window.preparePaste('line\n'), 'line\r');
  window.setInputEnabled(true); listeners.data('after');
  assert.deepEqual(messages, [{ type: 'input', data: 'after', pane: 'shell', target: null }]);
  window.focusTerminal(); window.focusTerminal();
  assert.equal(focused, 2);
});

function cachedRenderer() {
  const messages = [], frames = [], timers = new Map(), animations = [], renders = new Set(), events = {};
  let terminal, nextTimer = 0;
  class Element {
    children = []; classes = new Set(); textContent = '';
    style = { setProperty(name, value) { this[name] = value; } }; width = 100; height = 100; left = 0; top = 0;
    classList = { add: value => this.classes.add(value), remove: value => this.classes.delete(value) };
    addEventListener() {}
    setAttribute(name, value) { this[name] = value; }
    removeAttribute(name) { delete this[name]; }
    appendChild(child) { this.children.push(child); child.parent = this; }
    remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
    cloneNode(deep) {
      const copy = new Element(); copy.textContent = this.textContent; copy.width = this.width; copy.height = this.height;
      copy.id = this.id; copy.tagName = this.tagName;
      copy.className = this.className;
      if (deep) this.children.forEach(child => copy.appendChild(child.cloneNode(true)));
      return copy;
    }
    querySelectorAll(selector) {
      return this.children.flatMap(child => [
        ...((selector === '[id]' ? !!child.id : selector === 'canvas' ? child.tagName === 'CANVAS'
          : selector.split(', ').some(part => part === 'textarea' ? child.tagName === 'TEXTAREA' : child.className === part.slice(1))) ? [child] : []),
        ...child.querySelectorAll(selector),
      ]);
    }
    querySelector(selector) { return this.querySelectorAll(selector)[0]; }
    getContext() { return { drawImage: original => { this.pixels = original.pixels; } }; }
    get firstElementChild() { return this.children[0]; }
    getBoundingClientRect() { return { width: this.width, height: this.height, left: this.left, top: this.top }; }
    animate(keyframes, options) {
      const animation = { keyframes, options, cancelled: false, cancel() { this.cancelled = true; } };
      animations.push(animation); return animation;
    }
  }
  const root = new Element(), screen = new Element(), canvas = new Element(), body = new Element();
  const helpers = new Element(), textarea = new Element(), composition = new Element();
  helpers.className = 'xterm-helpers'; textarea.tagName = 'TEXTAREA'; composition.className = 'composition-view';
  helpers.appendChild(textarea); helpers.appendChild(composition); screen.appendChild(helpers);
  canvas.id = 'window'; canvas.appendChild(screen);
  const viewport = { clientWidth: 0, clientHeight: 0, scrollLeft: 13, scrollTop: 29,
    addEventListener(name, callback) { (events[name] ||= []).push(callback); } };
  screen.textContent = 'previous screen';
  root.querySelector = selector => selector === '.xterm-screen' ? screen : selector === '.xterm-helpers' ? helpers : null;
  class Terminal {
    element = root; cols = 10; rows = 5; parser = { registerOscHandler() {} };
    buffer = { active: { cursorX: 3, cursorY: 2, baseY: 10, viewportY: 10 } };
    constructor() { terminal = this; }
    loadAddon() {} open() {} onData(callback) { this.data = callback; } onBinary() {} onResize() {} refresh() {}
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
    onRender(callback) { renders.add(callback); return { dispose() { renders.delete(callback); } }; }
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
  return { window, root, screen, canvas, body, Element, messages, animations, helpers, textarea, composition, terminal, viewport,
    event(name, value) { for (const callback of events[name] || []) callback(value); },
    frame() { frames.splice(0).forEach(callback => callback()); },
    render() { for (const callback of [...renders]) callback(); },
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

test('first snapshot without a cache still acknowledges only after render and paint', () => {
  const r = cachedRenderer();
  r.window.endSnapshot(71);
  assert.deepEqual(r.messages, [], 'parsing alone must not uncover a switching window');
  r.frame(); r.frame();
  assert.deepEqual(r.messages, [], 'paint callbacks cannot replace a render event');
  r.render(); r.frame();
  assert.deepEqual(r.messages, []);
  r.frame();
  assert.deepEqual(r.messages, [{ type: 'ack', id: 71 }]);
  assert.equal(r.root.children.length, 0);
});

test('cancelled first-render callbacks cannot complete a newer snapshot', () => {
  const r = cachedRenderer();
  r.window.endSnapshot(72); r.render();
  r.window.cancelSnapshot();
  assert.deepEqual(r.messages.splice(0), [{ type: 'ack', id: 72 }]);
  r.window.endSnapshot(73);
  r.frame(); r.frame();
  assert.deepEqual(r.messages, [], 'old paint frames cannot release the successor');
  r.render(); r.frame(); r.frame();
  assert.deepEqual(r.messages, [{ type: 'ack', id: 73 }]);
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

function imeRenderer({ delayedFont = false, nativeViewport = false, keyboardMode = 'pan' } = {}) {
  const messages = [], frames = [], terminals = [], observed = new Set();
  const root = { clientWidth: 1000, clientHeight: 600, addEventListener() {} };
  const host = () => ({ style: {}, addEventListener() {}, appendChild() {}, setAttribute() {}, remove() {}, classList: { toggle() {} } });
  const canvas = { style: {}, appendChild() {} };
  let observe, render, terminal;
  class Terminal {
    cols = 100; rows = 30; options = {}; parser = { registerOscHandler() {} };
    measuredFont = 14;
    buffer = { active: { viewportY: 10, baseY: 10 } };
    screen = { getBoundingClientRect: () => ({ width: this.cols * 10 * (delayedFont ? this.measuredFont : this.options.fontSize || 14) / 14,
      height: this.rows * 20 * (delayedFont ? this.measuredFont : this.options.fontSize || 14) / 14 }) };
    element = { querySelector: selector => selector === '.xterm-screen' ? this.screen : null };
    constructor() { terminal ||= this; terminals.push(this); }
    loadAddon() {} open() {} dispose() {}
    onData(callback) { this.data = callback; } onBinary(callback) { this.binary = callback; }
    onRender(callback) { this.render = callback; render = callback; }
    onScroll(callback) { this.scroll = callback; }
    resize(columns, rows) { this.cols = columns; this.rows = rows; }
  }
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, FitAddon: { FitAddon: class {} },
    document: { body: Object.assign(host(), { dataset: { nativeViewport: String(nativeViewport) } }), getElementById: id => id === 'terminal' ? root : id === 'window' ? canvas : host(), createElement: host, hasFocus: () => false },
    ResizeObserver: class { constructor(callback) { observe = callback; } observe(element) { observed.add(element); observe(); } unobserve(element) { observed.delete(element); } },
    requestAnimationFrame: callback => frames.push(callback),
    clearTimeout() {},
  });
  window.configure(14, 5000, keyboardMode);
  messages.length = 0;
  return { window, root, canvas, messages, terminal, terminals, observed, observe: () => observe(), render: (t = terminals.at(-1)) => t.render(),
    frame() { frames.splice(0).forEach(callback => callback()); } };
}

function normalBuffer(lines = [], cursorY = Math.max(0, lines.length - 1)) {
  return { type: 'normal', viewportY: 10, baseY: 10, cursorY, cursorX: 0,
    getLine(y) { return { translateToString: () => lines[y - 10] || '' }; } };
}

test('pan uses the last content or cursor row and ignores only trailing empty rows', () => {
  for (const [lines, cursor, offset] of [
    [['한글😀', '', 'prompt', '   ', ''], 2, 0],
    [Array(15).fill('wrapped output'), 14, 0],
    [Array(16).fill('wrapped output'), 15, 20],
    [['first', ...Array(14).fill(''), 'last'], 1, 20],
    [['prompt'], 15, 20],
  ]) {
    const r = imeRenderer();
    r.terminal.buffer.active = normalBuffer(lines, cursor); r.render();
    r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
    assert.equal(r.canvas.style.transform, offset ? `translateY(-${offset}px)` : '');
    assert.deepEqual(r.messages, [], 'pan never changes remote rows');
  }
});

test('pan follows new output only when already at the content bottom and leaves history untouched', () => {
  const r = imeRenderer();
  r.terminal.buffer.active = normalBuffer(['prompt']); r.render();
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.terminal.buffer.active = normalBuffer(Array(20).fill('line')); r.render();
  assert.equal(r.canvas.style.transform, 'translateY(-100px)');
  r.terminal.buffer.active.viewportY = 3; r.terminal.scroll();
  assert.equal(r.terminal.buffer.active.viewportY, 3);
  assert.deepEqual(r.messages, []);
});

test('auto chooses resize once after IME animation and restores the full grid on close', () => {
  const r = imeRenderer({ keyboardMode: 'auto' });
  r.terminal.buffer.active = normalBuffer(['short', '', 'prompt']); r.render();
  const geometry = (version, visibleHeight, imeVisible = true) => r.window.setViewport({ version, width: 1000, logicalHeight: 600, visibleHeight, imeVisible });
  geometry(1, 600, false);
  r.window.setImeAnimation(true, 1);
  for (const [i, height] of [540, 420, 300].entries()) geometry(i + 2, height);
  assert.deepEqual(r.messages, [], 'animation frames must not resize the remote');
  r.window.setImeAnimation(false, 1); r.frame();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 15 }]);
  assert.equal(r.canvas.style.transform, '');
  r.window.resizePane(100, 15);
  r.terminal.buffer.active = normalBuffer(Array(15).fill('new output')); r.render();
  assert.deepEqual(r.messages, [], 'output growth cannot change the selected strategy');
  r.window.setImeAnimation(true, 2); geometry(5, 600, false);
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 2); r.frame();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 30 }]);
});

test('auto preserves long output alternate screens history and unknown content', () => {
  for (const buffer of [normalBuffer(Array(16).fill('line')), { ...normalBuffer(['prompt']), type: 'alternate' },
    { ...normalBuffer(['prompt']), viewportY: 9 }, { type: 'normal', baseY: 0, viewportY: 0 }]) {
    const r = imeRenderer({ keyboardMode: 'auto' });
    r.terminal.buffer.active = buffer; r.render();
    r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
    assert.deepEqual(r.messages, []);
    assert.equal(r.terminal.rows, 30);
    if (buffer.type === 'alternate') assert.equal(r.canvas.style.transform, 'translateY(-300px)');
  }
});

test('auto uses complete visible rows at the fit boundary', () => {
  for (const [rows, expected] of [[15, [{ type: 'resize', columns: 100, rows: 15 }]], [16, []]]) {
    const r = imeRenderer({ keyboardMode: 'auto' });
    r.terminal.buffer.active = normalBuffer(Array(rows).fill('line')); r.render();
    r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 319, imeVisible: true });
    assert.deepEqual(r.messages, expected);
  }
});

test('auto restores the full grid for a newly entered TUI and keeps pan until IME closes', () => {
  const r = imeRenderer({ keyboardMode: 'auto' });
  r.terminal.buffer.active = normalBuffer(['prompt']); r.render();
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.messages.length = 0; r.window.resizePane(100, 15);
  r.terminal.buffer.active.type = 'alternate'; r.render();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 30 }]);
  r.window.resizePane(100, 30); r.render();
  assert.equal(r.canvas.style.transform, 'translateY(-300px)');
  r.terminal.buffer.active = normalBuffer(['prompt']); r.render();
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 600, visibleHeight: 320, imeVisible: true });
  assert.deepEqual(r.messages, [], 'leaving the TUI must not oscillate back to resize');
});

test('explicit resize includes TUI and changes only the final height after rapid IME reversal', () => {
  const r = imeRenderer({ keyboardMode: 'resize' });
  r.terminal.buffer.active = { ...normalBuffer(), type: 'alternate' }; r.render();
  const geometry = (version, visibleHeight) => r.window.setViewport({ version, width: 1000, logicalHeight: 600, visibleHeight, imeVisible: visibleHeight < 600 });
  geometry(1, 600); r.window.setImeAnimation(true, 1); geometry(2, 300);
  r.window.setImeAnimation(true, 2); geometry(3, 600);
  r.window.setImeAnimation(false, 1); r.frame();
  assert.deepEqual(r.messages, []);
  r.window.setImeAnimation(false, 2); r.frame();
  assert.deepEqual(r.messages, []);
  geometry(4, 300);
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 15 }]);
  assert.equal(r.canvas.style.transform, '');
  r.window.setImeAnimation(true, 3); geometry(5, 400);
  r.window.setImeAnimation(false, 3); r.frame();
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 20 }]);
});

test('mode changes apply while IME is open and unknown settings fall back to auto', () => {
  const r = imeRenderer();
  r.terminal.buffer.active = normalBuffer(['prompt']); r.render();
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.window.configure(14, 5000, 'resize');
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 15 }]);
  r.window.configure(14, 5000, 'pan');
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 30 }]);
  r.window.setInputEnabled(false); // The settings dialog owns the input gate.
  r.window.configure(14, 5000, 'future-setting');
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 15 }]);
});

test('auto measures all visible tmux panes with their vertical positions and ignores hidden panes', () => {
  const r = imeRenderer({ keyboardMode: 'auto' });
  const layout = { window: '@1', columns: 100, rows: 30, active: '%1', panes: [
    { id: '%1', visible: true, left: 0, top: 0, width: 49, height: 30 },
    { id: '%2', visible: true, left: 50, top: 0, width: 50, height: 30 },
    { id: '%3', visible: false, left: 0, top: 0, width: 100, height: 30 },
  ] };
  r.window.setLayout(layout);
  for (const t of r.terminals.slice(1)) { t.buffer.active = normalBuffer(['prompt']); r.render(t); }
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 15 }]);
  r.window.setLayout({ ...layout, window: '@2', panes: layout.panes.map(p => p.id === '%2' ? { ...p, top: 20 } : p) });
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 30 }]);
  assert.equal(r.canvas.style.transform, 'translateY(-120px)');
});

test('auto excludes size generations from target changes and reselects for a new destination', () => {
  const r = imeRenderer({ keyboardMode: 'auto' });
  const layout = { window: null, active: 'shell', panes: [], inputTarget: 'size-1', inputIdentity: 'destination-1' };
  r.window.setLayout(layout);
  r.terminal.buffer.active = normalBuffer(Array(20).fill('line')); r.render();
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.terminal.buffer.active = normalBuffer(['short now']); r.render();
  r.window.setLayout({ ...layout, inputTarget: 'size-2' });
  assert.deepEqual(r.messages, [], 'output and size-only updates cannot restart auto selection');
  r.window.setLayout({ ...layout, inputTarget: 'new', inputIdentity: 'destination-2' });
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 100, rows: 15 }]);
});

test('auto cannot resize from snapshot intermediates or recovery caches', () => {
  for (const block of [r => r.window.beginSnapshot(), r => r.window.setConnectionRecovery(true)]) {
    const r = cachedRenderer();
    r.viewport.clientWidth = 800; r.viewport.clientHeight = 600;
    r.screen.width = 800; r.screen.height = 600; r.terminal.cols = 80; r.terminal.rows = 30;
    r.terminal.buffer.active = normalBuffer(['prompt']); r.render();
    r.window.setViewport({ version: 1, width: 800, logicalHeight: 600, visibleHeight: 600, imeVisible: false });
    r.messages.length = 0;
    block(r);
    r.window.setViewport({ version: 2, width: 800, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
    assert.deepEqual(r.messages, []);
  }
});

test('queued resize animation endings cannot escape a detached or superseded surface', () => {
  const r = imeRenderer({ keyboardMode: 'resize' });
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 600, imeVisible: false });
  r.window.setImeAnimation(true, 1);
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.window.setImeAnimation(false, 1);
  r.window.detachViewport(3); r.frame();
  assert.deepEqual(r.messages, []);
  r.window.setViewport({ version: 4, width: 800, logicalHeight: 400, visibleHeight: 200, imeVisible: true });
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 80, rows: 10 }]);
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  r.frame();
  assert.equal(r.messages.length, 1);
});

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

test('screen geometry observes late font metrics without a hidden shell render and after redisplay', () => {
  const r = imeRenderer({ delayedFont: true });
  r.window.setLayout({ window: '@1', columns: 100, rows: 30, active: '%1', panes: [
    { id: '%1', visible: true, left: 0, top: 0, width: 100, height: 30 },
  ] });
  const visible = r.terminals[1];
  assert.ok(r.observed.has(r.root));
  assert.ok(r.observed.has(r.terminal.screen));
  assert.ok(r.observed.has(visible.screen));
  r.window.configure(28, 5000); r.frame();
  assert.deepEqual(r.messages, [], 'options setters may return before measurements change');
  visible.measuredFont = 28;
  r.observe();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 50, rows: 15 }]);
  assert.equal(r.terminal.measuredFont, 14, 'the hidden measurement shell can remain stale');
  r.root.clientWidth = 0; r.root.clientHeight = 0;
  r.window.configure(14, 5000); visible.measuredFont = 14; r.observe();
  assert.deepEqual(r.messages, []);
  r.root.clientWidth = 800; r.root.clientHeight = 400; r.observe();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 80, rows: 20 }]);
  r.window.setLayout({ window: null, active: 'shell', panes: [] });
  assert.ok(!r.observed.has(visible.screen), 'disposed panes release their observer');
});

test('detached surfaces and older geometry generations cannot resize or move the panel', () => {
  const r = imeRenderer();
  r.window.setViewport({ version: 8, width: 1000, logicalHeight: 600, visibleHeight: 400, imeVisible: true });
  assert.equal(r.canvas.style.transform, 'translateY(-200px)');
  r.window.detachViewport(9);
  r.window.setViewport({ version: 8, width: 500, logicalHeight: 400, visibleHeight: 300, imeVisible: true });
  r.root.clientHeight = 200; r.observe(); r.window.refreshViewport();
  assert.deepEqual(r.messages, []);
  r.window.setViewport({ version: 10, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  assert.equal(r.canvas.style.transform, 'translateY(-300px)');
  assert.deepEqual(r.messages, [], 'reattachment reuses the same logical grid');
});

test('composition and textarea stay at the displayed cursor through snapshot parsing and final render', () => {
  const r = cachedRenderer();
  const position = () => [r.helpers.style['--dgl-input-left'], r.helpers.style['--dgl-input-top']];
  r.event('compositionstart');
  assert.deepEqual(position(), ['30px', '40px']);
  const cursor = new r.Element(); cursor.className = 'xterm-cursor';
  cursor.left = 64; cursor.top = 83; cursor.width = 10; cursor.height = 20;
  r.helpers.left = 4; r.helpers.top = 3;
  r.screen.appendChild(cursor);
  r.event('compositionupdate', { data: '한' });
  assert.deepEqual(position(), ['60px', '80px'], 'use the actual displayed cursor relative to the helpers');
  r.composition.textContent = '한';
  r.window.beginSnapshot();
  assert.equal(r.root.children[0].querySelectorAll('.xterm-helpers, .composition-view, textarea').length, 0);
  cursor.left = 4; cursor.top = 3;
  r.terminal.buffer.active.cursorX = 0; r.terminal.buffer.active.cursorY = 0;
  r.render(); r.event('compositionupdate', { data: '한' });
  assert.deepEqual(position(), ['60px', '80px'], 'a snapshot reset cursor must not move the live composition');
  r.window.beginSnapshot(); // a superseding snapshot keeps the same anchor
  r.window.endSnapshot(30); r.render(); r.frame();
  assert.deepEqual(position(), ['60px', '80px']);
  cursor.left = 24; cursor.top = 63;
  r.frame();
  assert.deepEqual(position(), ['20px', '60px']);
  assert.deepEqual(r.messages, [{ type: 'ack', id: 30 }]);
  assert.equal(r.composition.textContent, '한', 'positioning never edits xterm composition text');
});

test('composition fallback accounts for viewport history and connection caches contain no input helpers', () => {
  const r = cachedRenderer();
  r.terminal.buffer.active.viewportY = 9;
  r.event('compositionstart');
  assert.equal(r.helpers.style['--dgl-input-top'], '60px');
  r.window.setConnectionRecovery(true);
  assert.equal(r.body.children[0].querySelectorAll('.xterm-helpers, .composition-view, textarea').length, 0);
  r.window.beginSnapshot(); r.terminal.buffer.active.cursorX = 7;
  r.window.cancelSnapshot();
  assert.equal(r.helpers.style['--dgl-input-left'], '70px', 'cancellation releases the position freeze');
});

test('pinch and real window changes use logical geometry while IME only changes the clip', () => {
  const r = imeRenderer();
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 400, imeVisible: true });
  r.window.setFontGesture(true); r.window.configure(28, 5000);
  r.window.setImeAnimation(true, 1); r.root.clientHeight = 300; r.observe();
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  assert.deepEqual(r.messages, []);
  r.window.setFontGesture(false); r.render();
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 50, rows: 15 }]);
  r.window.setImeAnimation(false, 1); r.frame();
  assert.deepEqual(r.messages, []);
  r.window.setViewport({ version: 3, width: 800, logicalHeight: 400, visibleHeight: 200, imeVisible: true });
  assert.deepEqual(r.messages, [{ type: 'resize', columns: 40, rows: 10 }]);
});

test('IME show hide height changes and rapid reversal never request a remote resize', () => {
  const r = imeRenderer();
  let version = 1;
  const geometry = (visibleHeight, imeVisible) => r.window.setViewport({ version: version++, width: 1000, logicalHeight: 600, visibleHeight, imeVisible });
  geometry(600, false);
  r.window.setImeAnimation(true, 1);
  for (const height of [540, 480, 420, 360]) { geometry(height, true); r.root.clientHeight = height; r.observe(); }
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  assert.equal(r.canvas.style.height, '600px');
  r.window.setImeAnimation(false, 1); r.frame();
  geometry(300, true);
  assert.equal(r.canvas.style.transform, 'translateY(-300px)');
  r.window.setImeAnimation(true, 2);
  for (const height of [420, 480, 540, 600]) { geometry(height, height !== 600); r.root.clientHeight = height; r.observe(); }
  r.window.setImeAnimation(false, 1); // stale animation ending
  r.window.setImeAnimation(false, 2); r.frame();
  assert.equal(r.canvas.style.transform, '');
  geometry(360, true);
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  assert.deepEqual(r.messages, []);
});

test('IME panel alignment preserves the independent xterm history reading position', () => {
  const r = imeRenderer();
  r.terminal.buffer.active.viewportY = 3;
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 300, imeVisible: true });
  assert.equal(r.canvas.style.transform, 'translateY(-300px)');
  assert.equal(r.terminal.buffer.active.viewportY, 3);
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 600, visibleHeight: 600, imeVisible: false });
  assert.equal(r.canvas.style.transform, '');
  assert.equal(r.terminal.buffer.active.viewportY, 3);
  assert.deepEqual(r.messages, []);
});

test('IME clip updates skip cell measurement while real geometry and redisplay still measure', () => {
  const r = imeRenderer({ nativeViewport: true });
  let measurements = 0;
  const original = r.terminal.screen.getBoundingClientRect;
  r.terminal.screen.getBoundingClientRect = () => { measurements++; return original(); };
  let version = 0;
  const geometry = (width, logicalHeight, visibleHeight) => r.window.setViewport({
    version: ++version, width, logicalHeight, visibleHeight, imeVisible: visibleHeight < logicalHeight,
  });
  geometry(1000, 600, 600);
  const initial = measurements;
  for (const height of [540, 480, 300, 400, 350, 500, 600]) geometry(1000, 600, height);
  assert.equal(measurements, initial, 'IME frames only update CSS translation and scrollbar');
  geometry(800, 400, 200);
  assert.ok(measurements > initial, 'rotation and window changes still measure');
  const resized = measurements;
  r.window.detachViewport(++version); geometry(800, 400, 200);
  assert.ok(measurements > resized, 'reattachment must measure unchanged logical dimensions');
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

// Model pointer delivery and wheel mode routing; actual xterm encoding and DOM
// geometry are additionally covered by AndroidPlatformTest with the bundled JS.
function touchRenderer() {
  const messages = [], terminals = [], timers = [], frames = new Map();
  let now = 0, nextFrame = 0;
  class Element {
    style = {}; children = []; events = {}; classList = { toggle() {}, add() {}, remove() {} };
    clientWidth = 807; clientHeight = 480;
    addEventListener(name, callback, options) {
      (this.events[name] ||= []).push({ callback, capture: options === true || options?.capture === true });
    }
    dispatchEvent(event) {
      event.preventDefault ||= () => { event.defaultPrevented = true; };
      event.stopImmediatePropagation ||= () => { event.stopped = true; };
      for (const { callback } of [...this.events[event.type] || []].sort((a, b) => b.capture - a.capture)) {
        callback(event); if (event.stopped) break;
      }
      return !event.defaultPrevented;
    }
    appendChild(child) { this.children.push(child); }
    remove() {} setAttribute() {} removeAttribute() {}
    cloneNode() { return new Element(); } querySelectorAll() { return []; }
    getBoundingClientRect() { return { width: 807, height: 480 }; }
    setPointerCapture(id) { this.captured = id; }
    hasPointerCapture(id) { return this.captured === id; }
    releasePointerCapture() { this.captured = null; }
  }
  class Terminal {
    cols = 80; rows = 24; options = {}; parser = { registerOscHandler() {} };
    modes = { mouseTrackingMode: 'none' }; buffer = { active: { type: 'normal', viewportY: 50, baseY: 50 } };
    wheels = []; scrolls = []; focuses = 0;
    screen = { getBoundingClientRect: () => ({ left: 0, top: 0, right: this.cols * 10, bottom: this.rows * 20,
      width: this.cols * 10, height: this.rows * 20 }), cloneNode: () => new Element() };
    element = new Element();
    constructor() {
      terminals.push(this);
      this.element.querySelector = selector => selector === '.xterm-screen' ? this.screen : null;
      this.element.contains = () => true;
      this.element.addEventListener('wheel', event => {
        this.wheels.push(event);
        (this.binaryMode ? this.binary : this.data)(event.deltaY < 0 ? 'wheel-up' : 'wheel-down');
      });
    }
    open(host) { this.host = host; } loadAddon() {} dispose() {}
    onData(callback) { this.data = callback; } onBinary(callback) { this.binary = callback; }
    onKey(callback) { this.key = callback; } onRender(callback) { this.render = callback; }
    resize(cols, rows) { this.cols = cols; this.rows = rows; }
    focus() { this.focuses++; } scrollLines(lines) { this.scrolls.push(lines); this.buffer.active.viewportY += lines; }
  }
  const root = new Element(), canvas = new Element(), shell = new Element(), body = new Element();
  const window = { dgl: { postMessage: value => messages.push(JSON.parse(value)) } };
  vm.runInNewContext(readFileSync(new URL('../app/src/androidMain/assets/terminal/terminal.js', import.meta.url), 'utf8'), {
    window, Terminal, TextEncoder, WheelEvent: class {
      static DOM_DELTA_LINE = 1;
      constructor(type, options) { this.type = type; Object.assign(this, options); }
    },
    document: { getElementById: id => ({ terminal: root, window: canvas, shell })[id], createElement: () => new Element(),
      hasFocus: () => true, body },
    ResizeObserver: class { observe() {} unobserve() {} },
    setTimeout: callback => timers.push(callback), clearTimeout() {},
    requestAnimationFrame: callback => { frames.set(++nextFrame, callback); return nextFrame; },
    cancelAnimationFrame: id => frames.delete(id),
  });
  const layout = (target = 'one', active = 'shell', split = false) => window.setLayout({ window: split ? '@1' : null,
    columns: 81, rows: 24, active, inputTarget: target, panes: split ? [
      { id: '%1', visible: true, left: 0, top: 0, width: 40, height: 24 },
      { id: '%2', visible: true, left: 41, top: 0, width: 40, height: 24 },
    ] : [] });
  window.configure(12, 5000, 'pan');
  layout(); messages.length = 0;
  const pointer = (type, x, y, term = terminals[0], extra = {}) => {
    const event = { type, pointerType: 'touch', pointerId: 1, isPrimary: true, clientX: x, clientY: y, timeStamp: now, ...extra };
    term.host.dispatchEvent(event); return event;
  };
  return { window, terminals, messages, timers, layout, pointer, canvas, root, body,
    advance(ms) { now += ms; },
    frame(ms = 16) {
      now += ms;
      const pending = [...frames.values()]; frames.clear();
      pending.forEach(callback => callback(now));
    },
    pendingFrame() { return [...frames.values()].at(-1); },
    drag(from, to, term = terminals[0]) {
      pointer('pointerdown', 100, from, term); pointer('pointermove', 100, to, term); pointer('pointerup', 100, to, term);
    },
  };
}

test('touch scroll accumulates rows, reverses direction and ends without inertia or click input', () => {
  const r = touchRenderer(), t = r.terminals[0];
  r.pointer('pointerdown', 100, 100);
  assert.equal(r.pointer('pointermove', 100, 107).defaultPrevented, undefined);
  assert.equal(r.pointer('pointermove', 100, 108).defaultPrevented, true);
  assert.deepEqual(t.scrolls, []);
  r.pointer('pointermove', 100, 125); r.pointer('pointermove', 100, 141);
  r.pointer('pointermove', 100, 120);
  assert.deepEqual(t.scrolls, [-1, -1, 1]);
  r.pointer('pointerup', 100, 120); r.pointer('pointermove', 100, 180);
  for (const type of ['mousedown', 'mouseup', 'click']) assert.equal(r.pointer(type, 100, 180).stopped, true);
  assert.deepEqual(t.scrolls, [-1, -1, 1]);
  assert.equal(t.focuses, 0); assert.deepEqual(r.messages, []);
  r.pointer('pointerdown', 100, 100); r.pointer('pointerup', 100, 100); r.pointer('click', 100, 100);
  assert.equal(t.focuses, 1, 'a subsequent tap still focuses the terminal');
});

function startPanelFling() {
  const r = touchRenderer();
  r.window.setViewport({ version: 1, width: 807, logicalHeight: 480, visibleHeight: 80, imeVisible: true });
  r.messages.length = 0;
  r.pointer('pointerdown', 100, 100);
  r.advance(40); r.pointer('pointermove', 100, 140);
  r.advance(40); r.pointer('pointermove', 100, 180);
  r.pointer('pointerup', 100, 180);
  return r;
}

test('IME panel inertia follows the last 80ms with exponential decay and stops at a boundary', () => {
  const r = startPanelFling(), t = r.terminals[0];
  assert.equal(r.canvas.style.transform, 'translateY(-320px)');
  r.frame(250);
  const expected = 320 - 250 * (1 - Math.exp(-1));
  assert.equal(r.canvas.style.transform, `translateY(${-expected}px)`);
  t.data('\x1b[<64;1;1M'); t.binary('\x1b[M');
  for (const type of ['mousedown', 'mouseup', 'click', 'wheel']) assert.equal(r.pointer(type, 100, 180).stopped, true);
  r.frame(2000);
  const stopped = r.canvas.style.transform;
  assert.equal(r.pendingFrame(), undefined);
  r.frame(1000); assert.equal(r.canvas.style.transform, stopped);
  assert.deepEqual(t.scrolls, []); assert.deepEqual(t.wheels, []); assert.deepEqual(r.messages, []);

  const boundary = startPanelFling();
  boundary.pointer('pointerdown', 100, 100);
  boundary.advance(40); boundary.pointer('pointermove', 100, 300);
  boundary.pointer('pointerup', 100, 300); boundary.frame(250);
  assert.equal(boundary.canvas.style.transform, 'translateY(0px)');
  assert.equal(boundary.pendingFrame(), undefined);
});

test('IME panel inertia ignores old velocity and a stationary release', () => {
  const r = startPanelFling();
  r.pointer('pointerdown', 100, 100);
  r.advance(200); r.pointer('pointermove', 100, 200); // old fast movement falls outside the window
  r.advance(80); r.pointer('pointermove', 100, 208);
  r.pointer('pointerup', 100, 208);
  const offset = 320 - 108;
  r.frame(250);
  assert.equal(r.canvas.style.transform, `translateY(${-(offset - .1 * 250 * (1 - Math.exp(-1)))}px)`);
  r.pointer('pointerdown', 100, 100);
  r.advance(40); r.pointer('pointermove', 100, 120);
  r.advance(100); r.pointer('pointerup', 100, 120);
  assert.equal(r.pendingFrame(), undefined, 'holding still before release produces no fling');
});

test('IME panel inertia revokes queued frames on every input and destination cancellation', () => {
  const cancellations = [
    r => r.pointer('pointerdown', 100, 180),
    r => r.pointer('pointerdown', 100, 180, r.terminals[0], { pointerType: 'mouse' }),
    r => r.pointer('pointerdown', 100, 180, r.terminals[0], { isPrimary: false }),
    r => r.window.setImeAnimation(true, 1),
    r => r.window.setViewport({ version: 2, width: 807, logicalHeight: 480, visibleHeight: 120, imeVisible: true }),
    r => r.window.setViewport({ version: 2, width: 807, logicalHeight: 480, visibleHeight: 480, imeVisible: false }),
    r => r.layout('replacement'),
    r => r.layout('one', '%1', true),
    r => r.window.setLayout({ window: null, active: 'shell', inputTarget: 'one', columns: 81, rows: 24,
      panes: [{ id: '%1', left: 0, top: 0, width: 40, height: 24, visible: true }] }),
    r => r.window.beginSnapshot(),
    r => r.window.setInputEnabled(false),
    r => r.window.setPointerEnabled(false),
    r => r.window.setConnectionRecovery(true),
    r => r.window.setFontGesture(true),
    r => r.window.detachViewport(2),
    r => r.body.children[0].dispatchEvent({ type: 'pointerdown', pointerId: 5, clientY: 10 }),
  ];
  for (const cancel of cancellations) {
    const r = startPanelFling(), queued = r.pendingFrame();
    assert.equal(typeof queued, 'function');
    cancel(r);
    const offset = r.canvas.style.transform;
    queued(330); r.frame(250);
    assert.equal(r.canvas.style.transform, offset, cancel.toString());
    assert.deepEqual(r.messages.filter(m => m.type.startsWith('input')), []);
  }
});

test('taking pointer capture from an xterm cell does not cancel an active touch drag', () => {
  const r = touchRenderer(), t = r.terminals[0];
  r.pointer('pointerdown', 100, 100);
  r.pointer('pointermove', 100, 110, t, { isTrusted: true });
  assert.equal(t.host.captured, 1);
  r.pointer('lostpointercapture', 100, 110); // implicit child capture moved to host
  r.pointer('pointermove', 100, 140);
  assert.deepEqual(t.scrolls, [-2]);
  t.host.releasePointerCapture(); r.pointer('lostpointercapture', 100, 140);
  r.pointer('pointermove', 100, 180); r.pointer('pointerup', 100, 180);
  assert.deepEqual(t.scrolls, [-2], 'an actual loss still cancels the gesture');
});

test('fractional CSS row heights do not lose a step at the exact row boundary', () => {
  const r = touchRenderer(), t = r.terminals[0];
  const rect = t.screen.getBoundingClientRect();
  t.screen.getBoundingClientRect = () => ({ ...rect, height: 24 * 19.6, bottom: 24 * 19.6 });
  r.drag(100, 119.6); r.drag(100, 80.4);
  assert.deepEqual(t.scrolls, [-1, 1]);
});

test('touch TUI wheel uses row steps, captured target and raw channels without consuming keyboard or IME origins', () => {
  const r = touchRenderer(), t = r.terminals[0];
  t.modes.mouseTrackingMode = 'vt200';
  t.key({ domEvent: { key: 'c' } });
  r.drag(100, 141);
  assert.deepEqual(t.wheels.map(e => [e.deltaMode, e.deltaY]), [[1, -1], [1, -1]]);
  assert.deepEqual(r.messages.splice(0).map(m => [m.type, m.target, m.data]), [['input', 'one', 'wheel-up'], ['input', 'one', 'wheel-up']]);
  t.data('c'); assert.equal(r.messages.pop().type, 'keyboard');
  r.pointer('compositionstart');
  t.binaryMode = true; r.drag(100, 80);
  assert.deepEqual(r.messages.splice(0).map(m => [m.type, m.target, m.data]), [['input-binary', 'one', 'wheel-down']]);
  t.data('한'); assert.equal(r.messages.pop().text, '한');
  t.modes.mouseTrackingMode = 'none'; t.binaryMode = false; t.buffer.active.type = 'alternate';
  r.drag(100, 60);
  assert.deepEqual(r.messages.map(m => m.type), ['input', 'input'], 'alternate screen fallback remains in xterm, not the shortcut encoder');
  assert.deepEqual(t.scrolls, []);
  t.buffer.active.type = 'normal'; t.modes.mouseTrackingMode = 'x10'; r.drag(100, 80);
  assert.deepEqual(t.scrolls, [1], 'X10 does not support wheel reports');
});

test('inactive pane drag selects once without focus or scrolling, and remote split grids survive rendering', () => {
  const r = touchRenderer(); r.layout('one', '%1', true);
  const left = r.terminals[1], right = r.terminals[2];
  for (const pane of ['%1', '%2']) { r.window.outputPane(pane); r.window.resizePane(40, 24); }
  left.render(); right.render();
  assert.deepEqual([left.cols, right.cols], [40, 40]);
  assert.equal(r.canvas.style.width, '810px'); r.messages.length = 0;
  r.pointer('pointerdown', 100, 100, right); r.pointer('pointermove', 100, 140, right);
  assert.deepEqual(r.messages, [{ type: 'focus', pane: '%2', target: 'one' }]);
  r.layout('two', '%2', true);
  r.pointer('pointermove', 100, 180, right); r.pointer('pointerup', 100, 180, right); r.pointer('click', 100, 180, right);
  assert.equal(right.focuses, 0); assert.deepEqual(right.scrolls, []);
  r.drag(100, 140, right); assert.deepEqual(right.scrolls, [-2]);
});

test('horizontal drags and non-touch pointers stay outside vertical wheel handling', () => {
  const r = touchRenderer(), t = r.terminals[0];
  r.pointer('pointerdown', 100, 100);
  assert.equal(r.pointer('pointermove', 120, 110).defaultPrevented, undefined);
  r.pointer('pointermove', 120, 200); r.pointer('pointerup', 120, 200); r.pointer('click', 120, 200);
  assert.deepEqual(t.scrolls, []); assert.equal(t.focuses, 0);
  for (const pointerType of ['mouse', 'pen']) {
    r.pointer('pointerdown', 100, 100, t, { pointerType });
    r.pointer('pointermove', 100, 200, t, { pointerType });
    r.pointer('pointerup', 100, 200, t, { pointerType });
  }
  assert.deepEqual(t.scrolls, []);
});

test('target changes, disabled input, recovery, pinch and cancellation discard unsent touch movement', () => {
  for (const reason of ['target', 'disable', 'recovery', 'pinch', 'cancel', 'capture', 'second-pointer']) {
    const r = touchRenderer(), t = r.terminals[0];
    t.modes.mouseTrackingMode = 'any';
    r.pointer('pointerdown', 100, 100); r.pointer('pointermove', 100, 119);
    switch (reason) {
      case 'target': r.layout('two'); break;
      case 'disable': r.window.setInputEnabled(false); r.window.setInputEnabled(true); break;
      case 'recovery': r.window.setConnectionRecovery(true); r.window.setConnectionRecovery(false); break;
      case 'pinch': r.window.setFontGesture(true); r.window.setFontGesture(false); break;
      case 'cancel': r.pointer('pointercancel', 100, 119); break;
      case 'capture': r.pointer('lostpointercapture', 100, 119); break;
      case 'second-pointer': r.pointer('pointerdown', 110, 119, t, { pointerId: 2, isPrimary: false }); break;
    }
    r.pointer('pointermove', 100, 160); r.pointer('pointerup', 100, 160); r.pointer('click', 100, 160);
    assert.deepEqual(t.wheels, [], reason); assert.equal(t.focuses, 0, reason);
    r.drag(100, 119); assert.deepEqual(t.wheels, [], `${reason}: remainder must not survive into the next gesture`);
    r.drag(100, 120); assert.equal(t.wheels.length, 1, reason);
    assert.equal(r.messages.filter(m => m.type === 'input').at(-1).target, reason === 'target' ? 'two' : 'one');
  }
});

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
  assert.deepEqual(r.messages, [{ type: 'blocked-input' }, { type: 'blocked-input' }]);
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
  assert.equal(r.window.preparePaste('composer'), 'composer');
  assert.deepEqual(r.messages, [
    { type: 'input', data: '\x1b[0n', pane: 'shell', target: null },
    { type: 'keyboard', key: 'text', text: '한😀', control: false, shift: false, alt: false, applicationCursor: false, pane: 'shell', target: null },
    { type: 'input', data: 'pasted text', pane: 'shell', target: null },
    { type: 'input', data: '\x1b[<0;1;1M', pointer: true, pane: 'shell', target: null },
    { type: 'input-binary', data: '\x1b[M \x85\xff', pointer: true, pane: 'shell', target: null },
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


test('size synchronization preserves keyboard input while discarding coordinate reports', () => {
  const r = keyboardRenderer();
  r.window.setPointerEnabled(false);
  r.key('ArrowLeft', '\x1b[D');
  r.events.compositionstart(); r.events.compositionend(); r.listeners.data('한글');
  r.events.paste(); r.listeners.data('english paste');
  r.timers.splice(0).forEach(callback => callback());
  r.listeners.data('\x1b[<0;3;4M'); r.listeners.binary('\x1b[M !!');
  assert.equal(r.messages.length, 3);
  assert.equal(r.messages[0].key, 'left');
  assert.equal(r.messages[1].text, '한글');
  assert.equal(r.messages[2].data, 'english paste');
  r.window.setPointerEnabled(true);
  assert.equal(r.messages.length, 3, 'discarded pointer events are never replayed');
});


test('snapshot reset preserves cursor keys paste mode and composition until the replacement renders', () => {
  const r = cachedRenderer();
  r.terminal.modes = { applicationCursorKeysMode: true, bracketedPasteMode: true };
  r.event('compositionstart');
  r.window.beginSnapshot();
  r.terminal.modes = { applicationCursorKeysMode: false, bracketedPasteMode: false };
  r.window.sendShortcut('left', false, false, false);
  r.event('compositionend'); r.terminal.data('한글');
  assert.equal(r.messages[0].applicationCursor, true);
  assert.equal(r.messages[1].text, '한글');
  assert.equal(r.window.preparePaste('a\nb'), '\x1b[200~a\rb\x1b[201~');
  r.event('paste'); r.terminal.data('paste');
  assert.equal(r.messages[2].data, '\x1b[200~paste\x1b[201~');
  r.window.endSnapshot(42); r.render(); r.frame(); r.frame();
  r.window.sendShortcut('left', false, false, false);
  assert.equal(r.messages.at(-1).applicationCursor, false);
});

test('IME body drag moves the panel pixel for pixel with limits and no remote or history input', () => {
  const r = touchRenderer(), t = r.terminals[0];
  const geometry = (version, visibleHeight, imeVisible = true) => r.window.setViewport({ version, width: 807, logicalHeight: 480, visibleHeight, imeVisible });
  geometry(1, 240);
  assert.deepEqual(r.messages, [], 'pan keeps the previously measured remote grid');
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  assert.equal(r.canvas.style.width, '807px');
  const bar = r.body.children[0], thumb = bar.children[0];
  assert.equal(bar.style.display, 'block'); assert.equal(thumb.style.height, '120px'); assert.equal(thumb.style.top, '120px');
  t.modes.mouseTrackingMode = 'vt200';
  r.pointer('pointerdown', 100, 100); r.pointer('pointermove', 100, 107);
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  r.pointer('pointermove', 100, 140);
  assert.equal(r.canvas.style.transform, 'translateY(-200px)');
  assert.equal(thumb.style.top, '100px');
  t.data('\x1b[<64;1;1M'); t.binary('\x1b[M');
  r.pointer('pointermove', 100, 500); r.pointer('pointerup', 100, 500);
  assert.equal(r.canvas.style.transform, 'translateY(0px)');
  for (const type of ['mousedown', 'mouseup', 'click']) assert.equal(r.pointer(type, 100, 500).stopped, true);
  r.drag(400, -400);
  assert.equal(r.canvas.style.transform, 'translateY(-240px)');
  r.drag(100, 140); // leave panel away from bottom
  geometry(2, 480, false);
  assert.equal(r.canvas.style.transform, ''); assert.equal(bar.style.display, 'none');
  geometry(3, 240);
  assert.equal(r.canvas.style.transform, 'translateY(-240px)', 'reopening starts at the bottom');
  assert.deepEqual(t.scrolls, []); assert.deepEqual(t.wheels, []); assert.deepEqual(r.messages, []);
  geometry(4, 480, false); r.drag(100, 140);
  assert.equal(t.wheels.length, 2, 'closing IME restores existing TUI scrolling');
});

test('IME scrollbar scales thumb drag and discards cancellation target changes and pinch', () => {
  const r = touchRenderer();
  r.window.setViewport({ version: 1, width: 807, logicalHeight: 480, visibleHeight: 240, imeVisible: true });
  assert.deepEqual(r.messages, [], 'pan keeps the previously measured remote grid');
  const bar = r.body.children[0];
  const pointer = (type, y) => bar.dispatchEvent({ type, pointerId: 5, isPrimary: true, clientY: y });
  pointer('pointerdown', 160); pointer('pointermove', 110);
  assert.equal(r.canvas.style.transform, 'translateY(-140px)', '50px thumb travel moves 100px of panel');
  pointer('pointermove', -500); assert.equal(r.canvas.style.transform, 'translateY(0px)');
  pointer('pointercancel', -500); pointer('pointermove', 500);
  assert.equal(r.canvas.style.transform, 'translateY(0px)');
  pointer('pointerdown', 50); r.layout('replacement'); pointer('pointermove', 100);
  assert.equal(r.canvas.style.transform, 'translateY(0px)');
  pointer('pointerdown', 50); r.window.setFontGesture(true); pointer('pointermove', 100); r.window.setFontGesture(false);
  assert.equal(r.canvas.style.transform, 'translateY(0px)');
  r.pointer('pointerdown', 100, 100); r.pointer('pointermove', 180, 105);
  assert.equal(r.canvas.style.transform, 'translateY(0px)', 'horizontal gestures remain browser panning');
  assert.deepEqual(r.messages, []);
  const css = readFileSync(new URL('../app/src/androidMain/assets/terminal/style.css', import.meta.url), 'utf8');
  assert.match(css, /\.panel-scrollbar \{[^}]*width: 24px/);
  assert.match(css, /\.panel-scrollbar-thumb \{[^}]*width: 3px/);
});

test('content-based scrollbar preserves manual offset as output grows and removes trailing blank range', () => {
  const r = touchRenderer(), t = r.terminals[0];
  t.buffer.active = normalBuffer(Array(18).fill('line')); t.render();
  r.window.setViewport({ version: 1, width: 807, logicalHeight: 480, visibleHeight: 240, imeVisible: true });
  const bar = r.body.children[0], thumb = bar.children[0];
  assert.equal(r.canvas.style.transform, 'translateY(-120px)');
  assert.equal(thumb.style.height, '160px');
  r.drag(100, 140);
  assert.equal(r.canvas.style.transform, 'translateY(-80px)');
  t.buffer.active = normalBuffer(Array(20).fill('line')); t.render();
  assert.equal(r.canvas.style.transform, 'translateY(-80px)', 'manual position survives output growth');
  t.buffer.active = normalBuffer(['prompt']); t.render();
  assert.equal(r.canvas.style.transform, '');
  assert.equal(bar.style.display, 'none');
  assert.equal(t.buffer.active.viewportY, 10);
  assert.deepEqual(r.messages, []);
});

test('native WebView waits for logical geometry before measuring its initial or transient bounds', () => {
  const r = imeRenderer({ nativeViewport: true });
  r.root.clientHeight = 240; r.observe(); r.window.refreshViewport();
  assert.deepEqual(r.messages, [], 'IME-sized DOM bounds cannot escape before native geometry arrives');
  r.window.setViewport({ version: 1, width: 1000, logicalHeight: 600, visibleHeight: 240, imeVisible: true });
  assert.deepEqual(r.messages.splice(0), [{ type: 'resize', columns: 100, rows: 30 }]);
  r.window.setViewport({ version: 2, width: 0, logicalHeight: 600, visibleHeight: 240, imeVisible: true });
  r.window.setViewport({ version: 2, width: 1000, logicalHeight: 100, visibleHeight: 240, imeVisible: true });
  r.observe();
  assert.deepEqual(r.messages, [], 'invalid bridge geometry leaves the current grid intact');
});
