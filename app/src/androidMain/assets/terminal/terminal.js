'use strict';
(() => {
  const root = document.getElementById('terminal');
  const canvas = document.getElementById('window');
  const panes = new Map();
  let output = 'shell', active = 'shell', inputTarget = null, layout = null;
  let fontSize = 14, scrollback = 5000, enabled = true, smooth = false;
  const send = message => window.dgl.postMessage(JSON.stringify(message));
  // One rendered window, independent of pane snapshot timers and changing layouts.
  // Compose draws the 45% black dim and status above this frozen viewport.
  let recoveryScreen = null;
  window.setConnectionRecovery = active => {
    if (active !== true) {
      recoveryScreen?.remove(); recoveryScreen = null;
      return;
    }
    if (recoveryScreen) return;
    const previous = canvas.cloneNode(true);
    previous.removeAttribute('id');
    for (const element of previous.querySelectorAll('[id]')) {
      if (element.id === 'shell') element.classList.add('recovery-shell');
      element.removeAttribute('id');
    }
    // cloneNode does not copy a canvas backing store if a renderer uses one.
    const originals = canvas.querySelectorAll('canvas'), copies = previous.querySelectorAll('canvas');
    for (let i = 0; i < originals.length; ++i) {
      copies[i].width = originals[i].width; copies[i].height = originals[i].height;
      copies[i].getContext('2d')?.drawImage(originals[i], 0, 0);
    }
    const bounds = canvas.getBoundingClientRect();
    Object.assign(previous.style, {
      position: 'absolute', left: `${-root.scrollLeft}px`, top: `${-root.scrollTop}px`,
      width: `${bounds.width}px`, height: `${bounds.height}px`, minWidth: '100%', minHeight: '100%',
    });
    recoveryScreen = document.createElement('div');
    recoveryScreen.className = 'connection-cache';
    recoveryScreen.setAttribute('aria-hidden', 'true'); recoveryScreen.inert = true;
    recoveryScreen.appendChild(previous); document.body.appendChild(recoveryScreen);
  };
  function createPane(pane, host) {
  const api = {};
  const post = message => send(message.type === 'input' || message.type === 'input-binary'
    ? { ...message, pane, target: pane === active ? inputTarget : null } : message);
  const term = new Terminal({
    cursorBlink: true, scrollback: 5000, fontSize: 14,
    fontFamily: 'monospace', allowProposedApi: false,
    theme: { background: '#11151b', foreground: '#dce1e8' },
    linkHandler: { activate() {}, hover() {}, leave() {} },
  });
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(host);
  // The bundled xterm uses its DOM renderer. Keep just one rendered viewport,
  // not another terminal/scrollback buffer, while a streamed snapshot replaces it.
  let cachedScreen = null;
  let cacheVersion = 0;
  let cacheTimer = null;
  let renderListener = null;
  let renderAck = null;
  let smoothTransitions = false;
  let cachedSize = null;
  let animations = [];
  const stopAnimations = () => { animations.forEach(animation => animation.cancel()); animations = []; };
  api.setSmoothTransitions = enabled => {
    smoothTransitions = enabled === true;
    if (!smoothTransitions) stopAnimations();
  };
  const clearCache = () => {
    ++cacheVersion;
    clearTimeout(cacheTimer);
    renderListener?.dispose(); renderListener = null;
    stopAnimations();
    cachedScreen?.remove(); cachedScreen = null;
    cachedSize = null;
    if (renderAck !== null) post({ type: 'ack', id: renderAck });
    renderAck = null;
  };
  api.cancelSnapshot = clearCache;
  api.beginSnapshot = () => {
    const buffer = term.buffer?.active;
    if (buffer && buffer.viewportY < buffer.baseY) savedScroll = buffer.viewportY;
    const version = ++cacheVersion;
    if (renderAck !== null) post({ type: 'ack', id: renderAck });
    renderAck = null;
    clearTimeout(cacheTimer);
    stopAnimations();
    renderListener?.dispose(); renderListener = null;
    if (!cachedScreen) {
      const screen = term.element?.querySelector('.xterm-screen');
      if (!screen) return;
      cachedSize = screen.getBoundingClientRect();
      cachedScreen = document.createElement('div');
      cachedScreen.className = 'terminal-cache';
      cachedScreen.setAttribute('aria-hidden', 'true');
      cachedScreen.appendChild(screen.cloneNode(true));
      term.element.appendChild(cachedScreen);
    }
    cachedScreen.classList.remove('is-releasing');
    requestAnimationFrame(() => {
      if (cacheVersion === version) cachedScreen?.classList.add('is-updating');
    });
    // Error/background fallback: never leave a stale screen covering the terminal.
    cacheTimer = setTimeout(clearCache, 15000);
  };
  api.endSnapshot = id => {
    if (savedScroll !== null) { term.scrollToLine(savedScroll); savedScroll = null; }
    if (!Number.isSafeInteger(id) || id <= 0) return;
    if (!cachedScreen) { post({ type: 'ack', id }); return; }
    const version = cacheVersion;
    renderAck = id;
    renderListener?.dispose();
    renderListener = term.onRender(() => {
      renderListener?.dispose(); renderListener = null;
      // xterm's write callback only confirms parsing. Wait for its actual render
      // and a paint opportunity before exposing the replacement screen.
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (cacheVersion !== version || !cachedScreen) return;
        const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true;
        const duration = reducedMotion ? 0 : smoothTransitions ? 120 : 90;
        cachedScreen.style.transitionDuration = `${duration}ms`;
        const liveScreen = term.element.querySelector('.xterm-screen');
        const nextSize = liveScreen?.getBoundingClientRect();
        if (smoothTransitions && !reducedMotion && cachedSize?.width > 0 && cachedSize?.height > 0 &&
            nextSize?.width > 0 && nextSize?.height > 0 && typeof liveScreen.animate === 'function') {
          const options = { duration, easing: 'cubic-bezier(.2,.7,.2,1)' };
          animations.push(cachedScreen.firstElementChild.animate([
            { transformOrigin: 'top left', transform: 'scale(1)', filter: 'blur(0px)' },
            { transformOrigin: 'top left', transform: `scale(${nextSize.width / cachedSize.width},${nextSize.height / cachedSize.height})`, filter: 'blur(2px)' },
          ], options));
          animations.push(liveScreen.animate([
            { transformOrigin: 'top left', transform: `scale(${cachedSize.width / nextSize.width},${cachedSize.height / nextSize.height})`, opacity: 0, filter: 'blur(2px)' },
            { transformOrigin: 'top left', transform: 'scale(1)', opacity: 1, filter: 'blur(0px)' },
          ], options));
        }
        cachedScreen.classList.add('is-releasing');
        post({ type: 'ack', id }); renderAck = null;
        clearTimeout(cacheTimer);
        cacheTimer = setTimeout(() => { if (cacheVersion === version) clearCache(); }, duration);
      }));
    });
    term.refresh(0, term.rows - 1);
  };
  // Remote escape sequences cannot access the clipboard.
  term.parser.registerOscHandler(52, () => true);
  let inputEnabled = true;
  let pasteCapture = null;
  term.onData(data => {
    if (pasteCapture !== null) { pasteCapture.push(data); return; }
    if (inputEnabled && new TextEncoder().encode(data).length <= 65536) post({ type: 'input', data });
  });
  // Legacy mouse reports can contain bytes above 127; UTF-8 encoding corrupts them.
  term.onBinary(data => { if (inputEnabled && data.length <= 65536) post({ type: 'input-binary', data }); });
  api.setInputEnabled = enabled => { inputEnabled = enabled === true; };
  // Capture xterm's own paste transformation (including current bracketed-paste mode)
  // without emitting input. Kotlin validates the target and submits one acknowledged request.
  api.preparePaste = (text, enter) => {
    if (typeof text !== 'string' || text.length === 0) throw new Error('Empty paste');
    pasteCapture = [];
    try {
      term.paste(text);
      const data = pasteCapture.join('') + (enter === true ? '\r' : '');
      if (new TextEncoder().encode(data).length > 65536) throw new Error('Paste exceeds 64 KiB');
      return data;
    } finally { pasteCapture = null; }
  };
  let resizingRemote = false;

  api.resizePane = (columns, rows) => {
    if (!Number.isInteger(columns) || !Number.isInteger(rows) || columns < 1 || columns > 65535 || rows < 1 || rows > 65535) return;
    resizingRemote = true;
    try { term.resize(columns, rows); } finally { resizingRemote = false; }
  };
  const resize = () => { try { fit.fit(); } catch {} };

  api.configure = (fontSize, scrollback) => {
    if (Number.isInteger(fontSize) && fontSize >= 10 && fontSize <= 28) term.options.fontSize = fontSize;
    if (Number.isInteger(scrollback) && scrollback >= 100 && scrollback <= 10000) term.options.scrollback = scrollback;

  };
  let initialized = false;
  let savedScroll = null;
  api.receive = (base64, reset, id) => {
    if (typeof base64 !== 'string' || base64.length > 22000 || !Number.isSafeInteger(id) || id <= 0) return;
    let data = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
    if (reset === true) {
      if (initialized) {
        // Keep this pane's local history when replacing its viewport after resize.
        // The core's reset marker is RIS; substitute a mode reset and viewport clear.
        const prefix = new TextEncoder().encode('\x1b[?1049l\x1b[!p\x1b[0m\x1b[H\x1b[2J');
        const body = data[0] === 27 && data[1] === 99 ? data.subarray(2) : data;
        const replacement = new Uint8Array(prefix.length + body.length);
        replacement.set(prefix); replacement.set(body, prefix.length); data = replacement;
      } else { term.reset(); initialized = true; }
    }
    term.write(data, () => post({ type: 'ack', id }));
  };
  // xterm owns composition events; forwarding key/input events separately duplicates IME commits.
  // Focus after the WebView's default touch/mouse focus change. Doing this on
  // pointerdown lets the subsequent default click blur the textarea and hide IME.
  api.focusTerminal = () => term.focus();
  host.addEventListener('click', () => {
    if (!inputEnabled) return;
    if (pane !== active) send({ type: 'focus', pane, target: inputTarget });
    api.focusTerminal();
  });
  return { ...api, term, fit, host };
  }
  panes.set('shell', createPane('shell', document.getElementById('shell')));
  const current = () => panes.get(output) || panes.get('shell');
  const focused = () => panes.get(active) || panes.get('shell');
  for (const method of ['receive', 'resizePane', 'beginSnapshot', 'endSnapshot']) window[method] = (...args) => current()[method](...args);
  window.cancelSnapshot = () => { for (const p of panes.values()) p.cancelSnapshot(); };
  window.focusTerminal = () => focused().focusTerminal();
  window.preparePaste = (...args) => focused().preparePaste(...args);
  window.setInputEnabled = value => { enabled = value === true; for (const p of panes.values()) p.setInputEnabled(enabled); };
  window.setSmoothTransitions = value => { smooth = value === true; for (const p of panes.values()) p.setSmoothTransitions(smooth); };
  function ensure(id) {
    if (!panes.has(id)) {
      const host = document.createElement('div'); host.className = 'pane'; canvas.appendChild(host);
      const p = createPane(id, host); p.configure(fontSize, scrollback); p.setInputEnabled(enabled); p.setSmoothTransitions(smooth);
      panes.set(id, p);
    }
    return panes.get(id);
  }
  window.outputPane = (id, target) => { ensure(id); output = id; };
  let lastViewport = '';
  let imeAnimating = false, imeAnchoring = false, imeVersion = 0, preserveReading = false;
  let cell = { width: 8.54, height: 19.6 }, boundaries = [], draggingBoundary = false;
  function alignViewport() {
    if (!canvas.style) return;
    const height = layout?.window != null ? layout.rows * cell.height
      : focused().term.element?.querySelector('.xterm-screen')?.getBoundingClientRect()?.height;
    canvas.style.transform = imeAnchoring && !preserveReading && height > 0
      ? `translateY(${root.clientHeight - height}px)` : '';
  }
  window.setImeAnimation = (animating, version) => {
    if (!Number.isSafeInteger(version) || version < imeVersion) return;
    if (animating) {
      if (!imeAnchoring) {
        const buffer = focused().term.buffer?.active;
        preserveReading = !!buffer && buffer.viewportY < buffer.baseY;
      }
      imeVersion = version; imeAnimating = true; imeAnchoring = true;
      alignViewport();
    } else if (version === imeVersion) {
      // Wait for Compose's final layout. A replacement/cancelled animation owns a newer version.
      requestAnimationFrame(() => {
        if (version !== imeVersion) return;
        imeAnimating = false;
        const previous = lastViewport;
        resize();
        if (lastViewport === previous) { imeAnchoring = false; alignViewport(); }
      });
    }
  };
  function resize() {
    const shell = panes.get('shell');
    const rect = shell.term.element?.querySelector('.xterm-screen')?.getBoundingClientRect();
    if (!rect?.width || !rect?.height || !shell.term.cols || !shell.term.rows || root.clientWidth <= 0 || root.clientHeight <= 0) return;
    const measured = { width: rect.width / shell.term.cols, height: rect.height / shell.term.rows };
    const changed = measured.width !== cell.width || measured.height !== cell.height;
    cell = measured;
    const columns = Math.max(2, Math.min(500, Math.floor(root.clientWidth / cell.width)));
    const rows = Math.max(2, Math.min(250, Math.floor(root.clientHeight / cell.height)));
    const key = `${columns},${rows}`;
    alignViewport();
    if (imeAnimating) return;
    if (key !== lastViewport) { lastViewport = key; send({ type: 'resize', columns, rows }); arrange(); }
    else if (changed) arrange();
  }

  window.configure = (font, history) => {
    fontSize = font; scrollback = history;
    for (const p of panes.values()) p.configure(font, history);
    resize();
  };
  function arrange() {
    if (!layout || draggingBoundary) return;
    const shell = panes.get('shell');
    const rect = shell.term.element?.querySelector('.xterm-screen')?.getBoundingClientRect();
    if (rect?.width > 0 && rect?.height > 0) cell = { width: rect.width / shell.term.cols, height: rect.height / shell.term.rows };
    const tmux = layout.window !== null;
    shell.host.style.visibility = tmux ? 'hidden' : 'visible';
    boundaries.forEach(b => b.remove()); boundaries = [];
    canvas.style.width = tmux ? `${layout.columns * cell.width}px` : '100%';
    canvas.style.height = tmux ? `${layout.rows * cell.height}px` : '100%';
    if (!imeAnimating && `${layout.columns},${layout.rows}` === lastViewport) imeAnchoring = false;
    alignViewport();
    for (const [id, p] of panes) {
      if (id === 'shell') continue;
      const spec = layout.panes.find(p => p.id === id);
      p.host.style.display = spec?.visible ? 'block' : 'none';
      if (!spec) continue;
      Object.assign(p.host.style, { left: `${spec.left * cell.width}px`, top: `${spec.top * cell.height}px`, width: `${spec.width * cell.width}px`, height: `${spec.height * cell.height}px` });
      p.host.classList.toggle('active', id === active);
      if (!spec.visible) continue;
      for (const horizontal of [true, false]) {
        const edge = horizontal ? spec.left + spec.width : spec.top + spec.height;
        if (edge >= (horizontal ? layout.columns : layout.rows)) continue;
        const b = document.createElement('div'); b.className = 'boundary ' + (horizontal ? 'vertical' : 'horizontal');
        Object.assign(b.style, horizontal ? { left: `${edge * cell.width}px`, top: `${spec.top * cell.height}px`, width: `${cell.width}px`, height: `${spec.height * cell.height}px` }
          : { left: `${spec.left * cell.width}px`, top: `${edge * cell.height}px`, width: `${spec.width * cell.width}px`, height: `${cell.height}px` });
        b.addEventListener('pointerdown', e => {
          e.preventDefault(); b.setPointerCapture(e.pointerId); draggingBoundary = true;
          b.onpointercancel = () => { draggingBoundary = false; arrange(); };
          const start = horizontal ? e.clientX : e.clientY;
          const target = inputTarget;
          b.onpointerup = end => {
            const cells = ((horizontal ? end.clientX : end.clientY) - start) / (horizontal ? cell.width : cell.height);
            const delta = Math.sign(cells) * Math.round(Math.abs(cells));
            if (delta !== 0) send({ type: 'resize-pane', pane: id, target, columns: Math.max(1, Math.min(65535, spec.width + (horizontal ? delta : 0))), rows: Math.max(1, Math.min(65535, spec.height + (horizontal ? 0 : delta))) });
            b.onpointerup = null; draggingBoundary = false; arrange();
          };
        });
        canvas.appendChild(b); boundaries.push(b);
      }
    }
  }
  window.setLayout = value => {
    const ownedFocus = [...panes.values()].some(p => p.term.element?.contains?.(document.activeElement));
    const previous = active;
    layout = value; active = value.active || 'shell'; inputTarget = value.inputTarget;
    for (const p of value.panes) if (p.visible) ensure(p.id);
    for (const [id, p] of panes) if (id !== 'shell' && !value.panes.some(s => s.id === id)) {
      p.cancelSnapshot(); p.term.dispose(); p.host.remove(); panes.delete(id);
    }
    arrange();
    if (enabled && document.hasFocus() && ownedFocus && previous !== active) focused().focusTerminal();
  };
  // The server may keep a smaller grid than this viewport. Tapping the empty
  // container still opens the active pane's IME, just as tapping its cells does.
  root.addEventListener('click', event => {
    if (enabled && !event.target.closest('.pane, .boundary')) focused().focusTerminal();
  });
  panes.get('shell').term.onRender?.(resize);
  new ResizeObserver(resize).observe(root);
  send({ type: 'ready' });
})();
