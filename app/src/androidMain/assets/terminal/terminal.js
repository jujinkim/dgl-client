'use strict';
(() => {
  const root = document.getElementById('terminal');
  const canvas = document.getElementById('window');
  const panes = new Map();
  let output = 'shell', active = 'shell', inputTarget = null, layout = null;
  let fontSize = 12, scrollback = 5000, enabled = true, smooth = false;
  let fontGesture = false, pointerEnabled = true;
  window.setPointerEnabled = enabled => { pointerEnabled = enabled === true; if (!pointerEnabled) cancelTouchScroll(); };
  let touchSelection = null;
  let geometryObserver = null;
  const cancelTouchScroll = () => { for (const p of panes.values()) p.cancelTouchScroll(); };
  const send = message => window.dgl.postMessage(JSON.stringify(message));
  const stripInputHelpers = element => {
    for (const helper of element.querySelectorAll('.xterm-helpers, .composition-view, textarea')) helper.remove();
    return element;
  };
  // One rendered window, independent of pane snapshot timers and changing layouts.
  // Compose draws the 45% black dim and status above this frozen viewport.
  let recoveryScreen = null;
  window.setConnectionRecovery = active => {
    if (active !== true) {
      recoveryScreen?.remove(); recoveryScreen = null;
      return;
    }
    cancelTouchScroll();
    if (recoveryScreen) return;
    const previous = stripInputHelpers(canvas.cloneNode(true));
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
  const post = message => send(message.type === 'input' || message.type === 'input-binary' || message.type === 'keyboard'
    ? { ...message, pane, target: 'target' in message ? message.target : pane === active ? inputTarget : null } : message);
  const term = new Terminal({
    cursorBlink: true, scrollback: 5000, fontSize: 12,
    fontFamily: 'monospace', allowProposedApi: false,
    theme: { background: '#11151b', foreground: '#dce1e8' },
    linkHandler: { activate() {}, hover() {}, leave() {} },
  });
  term.open(host);
  // The bundled xterm uses its DOM renderer. Keep just one rendered viewport,
  // not another terminal/scrollback buffer, while a streamed snapshot replaces it.
  let cachedScreen = null;
  let cacheVersion = 0;
  let cacheTimer = null;
  let renderListener = null;
  let renderAck = null;
  let reconnectSnapshot = false;
  let smoothTransitions = false;
  let cachedSize = null;
  let animations = [];
  let snapshotUpdating = false, snapshotInputModes = null;
  // Use the rendered cursor, with public buffer coordinates as the fallback.
  // CSS variables take precedence over xterm's deferred composition positioning.
  const syncInputPosition = () => {
    if (snapshotUpdating) return;
    const screen = term.element?.querySelector('.xterm-screen');
    const helpers = term.element?.querySelector('.xterm-helpers');
    const buffer = term.buffer?.active;
    if (!screen || !helpers?.style?.setProperty || !buffer || !Number.isInteger(buffer.cursorY) || !term.cols || !term.rows) return;
    const bounds = screen.getBoundingClientRect(), origin = helpers.getBoundingClientRect();
    if (!(bounds.width > 0 && bounds.height > 0)) return;
    const width = bounds.width / term.cols, height = bounds.height / term.rows;
    const row = buffer.baseY + buffer.cursorY - buffer.viewportY;
    if (row < 0 || row >= term.rows) return;
    const cursor = screen.querySelector?.('.xterm-cursor')?.getBoundingClientRect();
    const left = cursor?.width > 0 ? cursor.left : bounds.left + Math.min(buffer.cursorX, term.cols - 1) * width;
    const top = cursor?.height > 0 ? cursor.top : bounds.top + row * height;
    helpers.style.setProperty('--dgl-input-left', `${left - origin.left}px`);
    helpers.style.setProperty('--dgl-input-top', `${top - origin.top}px`);
    helpers.style.setProperty('--dgl-input-height', `${height}px`);
  };
  api.syncInputPosition = syncInputPosition;
  term.onRender?.(() => { syncInputPosition(); resize(); });
  host.addEventListener('compositionstart', syncInputPosition, true);
  host.addEventListener('compositionupdate', syncInputPosition);
  host.addEventListener('focusin', syncInputPosition);
  const screen = term.element?.querySelector('.xterm-screen');
  if (screen) geometryObserver?.observe(screen);
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
    reconnectSnapshot = false;
    snapshotUpdating = false; snapshotInputModes = null;
    syncInputPosition();
    if (renderAck !== null) post({ type: 'ack', id: renderAck });
    renderAck = null;
  };
  api.cancelSnapshot = clearCache;
  api.beginSnapshot = () => {
    syncInputPosition();
    if (!snapshotUpdating) snapshotInputModes = { ...term.modes };
    snapshotUpdating = true;
    const buffer = term.buffer?.active;
    if (buffer && buffer.viewportY < buffer.baseY) savedScroll = buffer.viewportY;
    const version = ++cacheVersion;
    reconnectSnapshot = recoveryScreen !== null;
    if (renderAck !== null) post({ type: 'ack', id: renderAck });
    renderAck = null;
    clearTimeout(cacheTimer);
    stopAnimations();
    renderListener?.dispose(); renderListener = null;
    if (!cachedScreen) {
      const screen = term.element?.querySelector('.xterm-screen');
      if (!screen) { snapshotUpdating = false; return; }
      cachedSize = screen.getBoundingClientRect();
      cachedScreen = document.createElement('div');
      cachedScreen.className = 'terminal-cache';
      cachedScreen.setAttribute('aria-hidden', 'true');
      cachedScreen.appendChild(stripInputHelpers(screen.cloneNode(true)));
      term.element.appendChild(cachedScreen);
    }
    cachedScreen.classList.remove('is-releasing');
    cachedScreen.classList.remove('is-updating');
    cachedScreen.style.transitionDuration = '0ms';
    if (reconnectSnapshot) cachedScreen.classList.add('is-recovering');
    else cachedScreen.classList.remove('is-recovering');
    requestAnimationFrame(() => {
      if (cacheVersion === version && reconnectSnapshot) cachedScreen?.classList.add('is-updating');
    });
    // Error/background fallback: never leave a stale screen covering the terminal.
    cacheTimer = setTimeout(clearCache, 15000);
  };
  api.endSnapshot = id => {
    if (savedScroll !== null) { term.scrollToLine(savedScroll); savedScroll = null; }
    if (!Number.isSafeInteger(id) || id <= 0) return;
    if (!cachedScreen) { snapshotUpdating = false; snapshotInputModes = null; syncInputPosition(); post({ type: 'ack', id }); return; }
    const version = cacheVersion;
    renderAck = id;
    renderListener?.dispose();
    renderListener = term.onRender(() => {
      renderListener?.dispose(); renderListener = null;
      // xterm's write callback only confirms parsing. Wait for its actual render
      // and a paint opportunity before exposing the replacement screen.
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (cacheVersion !== version || !cachedScreen) return;
        snapshotUpdating = false; snapshotInputModes = null;
        syncInputPosition();
        const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true;
        // Resize, pane changes and ordinary snapshots swap atomically without motion.
        if (!reconnectSnapshot || reducedMotion) { clearCache(); return; }
        const duration = smoothTransitions ? 120 : 90;
        cachedScreen.style.transitionDuration = `${duration}ms`;
        const liveScreen = term.element.querySelector('.xterm-screen');
        const nextSize = liveScreen?.getBoundingClientRect();
        if (smoothTransitions && cachedSize?.width > 0 && cachedSize?.height > 0 &&
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
  let wheelOrigin = null;
  let keyOrigin = null, textOrigin = false, textTarget = null, composing = false, compositionPending = false, compositionVersion = 0, pasting = false;
  const keyboard = (key, text, modifiers = {}, target = pane === active ? inputTarget : null) => {
    if (fontGesture) return;
    if (!inputEnabled) { post({ type: 'blocked-input' }); return; }
    post({ type: 'keyboard', key, text, target, control: modifiers.ctrlKey === true,
      shift: modifiers.shiftKey === true, alt: modifiers.altKey === true,
      applicationCursor: (snapshotInputModes || term.modes)?.applicationCursorKeysMode === true });
  };
  // Public APIs only. onKey runs before the corresponding onData, so that event
  // is encoded from its semantic key rather than transforming terminal bytes.
  term.attachCustomKeyEventHandler?.(event => {
    if (event.type === 'keydown' && (event.isComposing || event.keyCode === 229)) {
      if (!compositionPending) textTarget = pane === active ? inputTarget : null;
      textOrigin = true;
    }
    return true;
  });
  term.onKey?.(({ domEvent }) => {
    // Clipboard/app shortcuts keep xterm/OS behavior. Sticky Ctrl is never Meta.
    if (domEvent.metaKey || (domEvent.ctrlKey && domEvent.shiftKey && ['c', 'v'].includes(domEvent.key.toLowerCase()))) return;
    keyOrigin = { event: domEvent, target: pane === active ? inputTarget : null };
  });
  host.addEventListener('compositionstart', () => { composing = true; compositionPending = true; ++compositionVersion; textOrigin = true; textTarget = pane === active ? inputTarget : null; keyOrigin = null; }, true);
  host.addEventListener('compositionend', () => {
    composing = false; textOrigin = true;
    const version = compositionVersion;
    // Bubble after xterm schedules its final delta. Clear canceled/empty origins
    // after that callback, without touching a newer composition.
    setTimeout(() => { if (!composing && version === compositionVersion) { compositionPending = false; textOrigin = false; } }, 0);
  });
  host.addEventListener('input', event => {
    if (event.inputType === 'insertFromPaste') return;
    if (event.data || event.inputType === 'insertText' || event.inputType === 'insertCompositionText') {
      if (!compositionPending) textTarget = pane === active ? inputTarget : null;
      textOrigin = true;
    }
  }, true);
  host.addEventListener('paste', () => {
    pasting = true; keyOrigin = null;
    setTimeout(() => { pasting = false; }, 0);
  }, true);
  api.sendShortcut = (key, control, shift, alt) => keyboard(key, null, { ctrlKey: control, shiftKey: shift, altKey: alt });
  term.onData(data => {
    if (pasteCapture !== null) { pasteCapture.push(data); return; }
    // Wheel reports belong to the gesture's destination, independently of a
    // pending key/IME origin. xterm emits these synchronously during dispatch.
    if (wheelOrigin) {
      if (inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !fontGesture && new TextEncoder().encode(data).length <= 65536)
        post({ type: 'input', data, target: wheelOrigin.target, pointer: true });
      return;
    }
    if (pasting && snapshotInputModes) {
      const currentBracketed = term.modes?.bracketedPasteMode === true;
      if (currentBracketed && data.startsWith('\x1b[200~') && data.endsWith('\x1b[201~')) data = data.slice(6, -6);
      if (snapshotInputModes.bracketedPasteMode) data = '\x1b[200~' + data + '\x1b[201~';
    }
    const captured = keyOrigin, origin = captured?.event; keyOrigin = null;
    if (new TextEncoder().encode(data).length > 65536) return;
    if (!pasting && origin) {
      const named = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
        PageUp: 'pageup', PageDown: 'pagedown', ' ': 'space' };
      const key = named[origin.key] || origin.key.toLowerCase();
      keyboard(key, [...origin.key].length === 1 ? origin.key : null, origin, captured.target);
      textOrigin = false; compositionPending = false;
    } else if (!pasting && textOrigin && !/[\x00-\x1f\x7f]/.test(data)) {
      // xterm emits only the finalized composition/textarea delta, once. Never
      // intercept intermediate IME updates or reinterpret protocol/mouse reports.
      keyboard('text', data, {}, textTarget); textOrigin = composing; compositionPending = composing;
    } else if (!pasting && textOrigin && ['\r', '\t', '\x7f'].includes(data)) {
      keyboard({ '\r': 'enter', '\t': 'tab', '\x7f': 'backspace' }[data], null, {}, textTarget); textOrigin = composing; compositionPending = composing;
    } else if (inputEnabled && !fontGesture) {
      const pointer = /^\x1b\[([<M])/.test(data);
      if (!pointer || (pointerEnabled && !snapshotUpdating && !imeAnimating)) post({ type: 'input', data, ...(pointer ? { pointer: true } : {}) });
    } else if (!fontGesture) post({ type: 'blocked-input' });
  });
  // Legacy mouse reports can contain bytes above 127; UTF-8 encoding corrupts them.
  term.onBinary(data => {
    if (inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !fontGesture && data.length <= 65536)
      post({ type: 'input-binary', data, pointer: true, target: wheelOrigin ? wheelOrigin.target : pane === active ? inputTarget : null });
  });
  api.setInputEnabled = enabled => {
    inputEnabled = enabled === true;
    if (!inputEnabled) api.cancelTouchScroll();
  };
  // Capture xterm's own paste transformation (including current bracketed-paste mode)
  // without emitting input. Kotlin validates the target and submits one acknowledged request.
  api.preparePaste = (text, enter) => {
    if (typeof text !== 'string' || text.length === 0) throw new Error('Empty paste');
    pasteCapture = [];
    try {
      if (snapshotInputModes) {
        const normalized = text.replace(/\r?\n/g, '\r');
        pasteCapture.push(snapshotInputModes.bracketedPasteMode ? '\x1b[200~' + normalized + '\x1b[201~' : normalized);
      } else term.paste(text);
      const data = pasteCapture.join('') + (enter === true ? '\r' : '');
      if (new TextEncoder().encode(data).length > 65536) throw new Error('Paste exceeds 64 KiB');
      return data;
    } finally { pasteCapture = null; }
  };
  api.resizePane = (columns, rows) => {
    if (!Number.isInteger(columns) || !Number.isInteger(rows) || columns < 1 || columns > 65535 || rows < 1 || rows > 65535) return;
    term.resize(columns, rows);
  };

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
  let touch = null, suppressTouchClick = false;
  api.cancelTouchScroll = () => {
    const previous = touch;
    touch = null;
    if (previous) {
      suppressTouchClick = true;
      if (host.hasPointerCapture?.(previous.id)) host.releasePointerCapture(previous.id);
    }
  };
  const canScroll = gesture => inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !fontGesture && !recoveryScreen &&
    !draggingBoundary && pane === active && gesture.target === inputTarget;
  host.addEventListener('pointerdown', event => {
    if (event.pointerType !== 'touch') { suppressTouchClick = false; return; }
    if (event.isPrimary === false || touch) { cancelTouchScroll(); return; }
    suppressTouchClick = false;
    touchSelection = null;
    if (!inputEnabled || !pointerEnabled || snapshotUpdating || imeAnimating || fontGesture || recoveryScreen || draggingBoundary) return;
    touch = { id: event.pointerId, x: event.clientX, y: event.clientY, lastY: event.clientY,
      remainder: 0, dragging: false, selected: pane === active, target: inputTarget };
  }, true);
  host.addEventListener('pointermove', event => {
    const gesture = touch;
    if (!gesture || event.pointerId !== gesture.id) return;
    const dx = event.clientX - gesture.x, dy = event.clientY - gesture.y;
    if (!gesture.dragging) {
      if (Math.abs(dx) >= 8 && Math.abs(dx) >= Math.abs(dy)) {
        // Let the browser pan the wider remote grid horizontally.
        api.cancelTouchScroll(); return;
      }
      if (Math.abs(dy) < 8 || Math.abs(dy) <= Math.abs(dx)) return;
      gesture.dragging = true; suppressTouchClick = true;
      if (event.isTrusted) host.setPointerCapture?.(gesture.id);
      if (!gesture.selected) {
        touchSelection = { pane, target: gesture.target };
        send({ type: 'focus', pane, target: gesture.target });
      }
    }
    event.preventDefault();
    if (!gesture.selected) return; // The first drag only selects this pane.
    if (!canScroll(gesture)) { api.cancelTouchScroll(); return; }
    const bounds = term.element?.querySelector('.xterm-screen')?.getBoundingClientRect();
    const rowHeight = bounds?.height / term.rows;
    if (!(rowHeight > 0)) { api.cancelTouchScroll(); return; }
    gesture.remainder += gesture.lastY - event.clientY;
    gesture.lastY = event.clientY;
    // CSS coordinates can differ by a few floating-point ulps at a row edge.
    const lines = Math.sign(gesture.remainder) * Math.floor((Math.abs(gesture.remainder) + 1e-6) / rowHeight);
    gesture.remainder -= lines * rowHeight;
    if (!lines) return;
    if (!['vt200', 'drag', 'any'].includes(term.modes?.mouseTrackingMode) && term.buffer?.active.type !== 'alternate') {
      term.scrollLines(lines);
      return;
    }
    // One line-mode wheel per row: xterm chooses SGR/binary mouse encoding or
    // application/normal cursor keys in a mouse-less alternate screen.
    wheelOrigin = { target: gesture.target };
    try {
      for (let i = 0; i < Math.abs(lines) && touch === gesture && canScroll(gesture); ++i) {
        term.element.dispatchEvent(new WheelEvent('wheel', { bubbles: true, cancelable: true,
          deltaMode: WheelEvent.DOM_DELTA_LINE, deltaY: Math.sign(lines),
          clientX: Math.max(bounds.left, Math.min(event.clientX, bounds.right - 1)),
          clientY: Math.max(bounds.top, Math.min(event.clientY, bounds.bottom - 1)) }));
      }
    } finally { wheelOrigin = null; }
  }, { capture: true, passive: false });
  host.addEventListener('pointerup', event => {
    if (touch?.id !== event.pointerId) return;
    if (touch.dragging) event.preventDefault();
    const previous = touch; touch = null;
    if (host.hasPointerCapture?.(previous.id)) host.releasePointerCapture(previous.id);
  }, true);
  for (const name of ['pointercancel', 'lostpointercapture']) host.addEventListener(name, event => {
    // Taking capture from xterm's implicitly captured cell bubbles a loss from
    // that child. Our host still owns the pointer, so the drag must continue.
    if (name === 'lostpointercapture' && host.hasPointerCapture?.(event.pointerId)) return;
    if (touch?.id === event.pointerId) api.cancelTouchScroll();
  }, true);
  // WebView may still synthesize mouse/click events after a consumed gesture.
  // Capture before xterm's mouse reporting and before our tap-to-focus handler.
  for (const name of ['mousedown', 'mouseup', 'click']) host.addEventListener(name, event => {
    if (suppressTouchClick) { event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
  host.addEventListener('click', () => {
    if (!inputEnabled || fontGesture) return;
    if (pane !== active) send({ type: 'focus', pane, target: inputTarget });
    api.focusTerminal();
  });
  return { ...api, term, host };
  }
  panes.set('shell', createPane('shell', document.getElementById('shell')));
  const current = () => panes.get(output) || panes.get('shell');
  const focused = () => panes.get(active) || panes.get('shell');
  for (const method of ['receive', 'beginSnapshot', 'endSnapshot']) window[method] = (...args) => current()[method](...args);
  window.resizePane = (...args) => { current().resizePane(...args); resize(); };
  window.cancelSnapshot = () => { for (const p of panes.values()) p.cancelSnapshot(); };
  window.focusTerminal = () => focused().focusTerminal();
  window.preparePaste = (...args) => focused().preparePaste(...args);
  window.sendShortcut = (...args) => focused().sendShortcut(...args);
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
  function sizeCanvas() {
    if (!canvas.style || root.clientWidth <= 0 || root.clientHeight <= 0) return;
    const tmux = layout?.window != null;
    const shell = panes.get('shell').term;
    const width = (tmux ? layout.columns : shell.cols) * cell.width;
    const height = (tmux ? layout.rows : shell.rows) * cell.height;
    if (!(width > 0 && height > 0)) return;
    canvas.style.width = `${Math.max(root.clientWidth, width)}px`;
    canvas.style.height = `${Math.max(root.clientHeight, height)}px`;
    root.scrollLeft = Math.max(0, Math.min(root.scrollLeft || 0, Math.max(0, width - root.clientWidth)));
    root.scrollTop = Math.max(0, Math.min(root.scrollTop || 0, Math.max(0, height - root.clientHeight)));
  }
  window.setFontGesture = value => {
    fontGesture = value === true;
    if (fontGesture) {
      cancelTouchScroll();
      // Native touch cancellation also cancels pointer capture. Clear a pending
      // boundary commit now, before any delayed WebView pointer event arrives.
      for (const b of boundaries) { b.onpointerup = null; b.onpointercancel = null; }
      draggingBoundary = false;
    }
    resize();
  };
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
    } else {
      imeVersion = version;
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
  window.refreshViewport = () => { lastViewport = ''; resize(); };
  function resize() {
    const term = focused().term;
    const rect = term.element?.querySelector('.xterm-screen')?.getBoundingClientRect();
    if (!rect?.width || !rect?.height || !term.cols || !term.rows || root.clientWidth <= 0 || root.clientHeight <= 0) return;
    const measured = { width: rect.width / term.cols, height: rect.height / term.rows };
    const changed = measured.width !== cell.width || measured.height !== cell.height;
    cell = measured;
    const columns = Math.max(2, Math.min(500, Math.floor(root.clientWidth / cell.width)));
    const rows = Math.max(2, Math.min(250, Math.floor(root.clientHeight / cell.height)));
    const key = `${columns},${rows}`;
    alignViewport();
    if (imeAnimating || fontGesture) {
      if (changed) arrange(); else sizeCanvas();
      return;
    }
    if (key !== lastViewport) { lastViewport = key; send({ type: 'resize', columns, rows }); arrange(); }
    else if (changed) arrange();
    else sizeCanvas();
  }

  window.configure = (font, history) => {
    fontSize = font; scrollback = history;
    for (const p of panes.values()) p.configure(font, history);
    resize();
    // Font metrics can settle after the options setter returns, including while
    // the measurement shell is hidden. Screen ResizeObservers catch later changes.
    requestAnimationFrame(resize);
  };
  function arrange() {
    if (draggingBoundary) return;
    const shell = panes.get('shell');
    sizeCanvas();
    if (!layout) return;
    const tmux = layout.window !== null;
    shell.host.style.visibility = tmux ? 'hidden' : 'visible';
    boundaries.forEach(b => b.remove()); boundaries = [];
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
          if (fontGesture || !pointerEnabled || imeAnimating) return;
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
    const selectedByTouch = touchSelection !== null && touchSelection.pane === value.active && touchSelection.target === inputTarget;
    if (active !== (value.active || 'shell') || inputTarget !== value.inputTarget || layout?.window !== value.window) {
      cancelTouchScroll(); touchSelection = null;
    }
    layout = value; active = value.active || 'shell'; inputTarget = value.inputTarget;
    for (const p of value.panes) if (p.visible) ensure(p.id);
    for (const [id, p] of panes) if (id !== 'shell' && !value.panes.some(s => s.id === id)) {
      const screen = p.term.element?.querySelector('.xterm-screen');
      if (screen) geometryObserver?.unobserve(screen);
      p.cancelSnapshot(); p.term.dispose(); p.host.remove(); panes.delete(id);
    }
    arrange();
    if (enabled && document.hasFocus() && ownedFocus && previous !== active && !selectedByTouch) focused().focusTerminal();
  };
  // The server may keep a smaller grid than this viewport. Tapping the empty
  // container still opens the active pane's IME, just as tapping its cells does.
  root.addEventListener('click', event => {
    if (enabled && !fontGesture && !event.target.closest('.pane, .boundary')) focused().focusTerminal();
  });
  geometryObserver = new ResizeObserver(() => {
    resize();
    for (const p of panes.values()) p.syncInputPosition();
  });
  geometryObserver.observe(root);
  for (const p of panes.values()) {
    const screen = p.term.element?.querySelector('.xterm-screen');
    if (screen) geometryObserver.observe(screen);
  }
  send({ type: 'ready' });
})();
