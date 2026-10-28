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
  const nativeViewport = document.body?.dataset?.nativeViewport === 'true';
  let viewport = null, viewportVersion = 0, viewportDetached = false;
  let keyboardMode = 'auto', keyboardChoice = 'pan', keyboardDecisionPending = true, autoPanLocked = false;
  let sizingHeight = null, panelHeight = 0, panelLimit = 0;
  let panelOffset = 0, panelDragging = false, panelBar = null, panelThumb = null, barDrag = null;
  let panelFling = null;
  const stopPanelFling = () => {
    const previous = panelFling; panelFling = null;
    if (previous && typeof cancelAnimationFrame === 'function') cancelAnimationFrame(previous.frame);
  };
  const panelRange = () => panelLimit;
  const logicalWidth = () => viewport?.width ?? root.clientWidth;
  const logicalHeight = () => viewport?.logicalHeight ?? root.clientHeight;
  const cancelTouchScroll = () => {
    stopPanelFling();
    for (const p of panes.values()) p.cancelTouchScroll();
    panelDragging = false;
    const previous = barDrag; barDrag = null;
    if (previous && panelBar?.hasPointerCapture?.(previous.id)) panelBar.releasePointerCapture(previous.id);
  };
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
  api.content = null;
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
  api.snapshotting = () => snapshotUpdating;
  api.refreshContent = () => { if (!snapshotUpdating) api.content = readPaneContent(term); };
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
  term.onRender?.(() => { api.refreshContent(); syncInputPosition(); resize(); });
  term.onScroll?.(() => { api.refreshContent(); resize(); });
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
    api.refreshContent();
    syncInputPosition();
    resize();
    if (renderAck !== null) post({ type: 'ack', id: renderAck });
    renderAck = null;
  };
  api.cancelSnapshot = clearCache;
  api.beginSnapshot = () => {
    api.cancelTouchScroll();
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
    const version = cacheVersion;
    renderAck = id;
    renderListener?.dispose();
    renderListener = term.onRender(() => {
      renderListener?.dispose(); renderListener = null;
      // xterm's write callback only confirms parsing. Wait for its actual render
      // and a paint opportunity before exposing the replacement screen.
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (cacheVersion !== version || renderAck !== id) return;
        snapshotUpdating = false; snapshotInputModes = null;
        api.refreshContent();
        resize();
        syncInputPosition();
        const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true;
        // Resize, pane changes and ordinary snapshots swap atomically without motion.
        if (!cachedScreen || !reconnectSnapshot || reducedMotion) { clearCache(); return; }
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
      if (inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !panelDragging && !panelFling && !fontGesture && new TextEncoder().encode(data).length <= 65536)
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
      if (!pointer || (pointerEnabled && !snapshotUpdating && !imeAnimating && !panelDragging && !panelFling)) post({ type: 'input', data, ...(pointer ? { pointer: true } : {}) });
    } else if (!fontGesture) post({ type: 'blocked-input' });
  });
  // Legacy mouse reports can contain bytes above 127; UTF-8 encoding corrupts them.
  term.onBinary(data => {
    if (inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !panelDragging && !panelFling && !fontGesture && data.length <= 65536)
      post({ type: 'input-binary', data, pointer: true, target: wheelOrigin ? wheelOrigin.target : pane === active ? inputTarget : null });
  });
  api.setInputEnabled = enabled => {
    inputEnabled = enabled === true;
    if (!inputEnabled) api.cancelTouchScroll();
  };
  // Capture xterm's own paste transformation (including current bracketed-paste mode)
  // without emitting input. Kotlin sends Enter separately after the paste request succeeds.
  api.preparePaste = text => {
    if (typeof text !== 'string' || text.length === 0) throw new Error('Empty paste');
    pasteCapture = [];
    try {
      if (snapshotInputModes) {
        const normalized = text.replace(/\r?\n/g, '\r');
        pasteCapture.push(snapshotInputModes.bracketedPasteMode ? '\x1b[200~' + normalized + '\x1b[201~' : normalized);
      } else term.paste(text);
      const data = pasteCapture.join('');
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
    stopPanelFling();
    const previous = touch;
    touch = null;
    if (previous) {
      panelDragging = false;
      suppressTouchClick = true;
      if (host.hasPointerCapture?.(previous.id)) host.releasePointerCapture(previous.id);
    }
  };
  const canScroll = gesture => inputEnabled && pointerEnabled && !snapshotUpdating && !imeAnimating && !fontGesture && !recoveryScreen &&
    !draggingBoundary && pane === active && gesture.target === inputTarget;
  const samplePanel = (gesture, event) => {
    if (!Number.isFinite(event.timeStamp)) return;
    const samples = gesture.samples;
    if (samples.length && event.timeStamp <= samples.at(-1).time) return;
    samples.push({ time: event.timeStamp, y: event.clientY });
    while (samples.length > 2 && samples[1].time <= event.timeStamp - 80) samples.shift();
  };
  const flingPanel = (gesture, event) => {
    samplePanel(gesture, event);
    const samples = gesture.samples, last = samples.at(-1), first = samples[0];
    if (!last || last.time <= first.time || !canScroll(gesture) || !panelRange()) return;
    const start = Math.max(first.time, last.time - 80);
    const next = samples[1];
    const y = start > first.time ? first.y + (next.y - first.y) * (start - first.time) / (next.time - first.time) : first.y;
    const velocity = (y - last.y) / (last.time - start); // CSS pixels/ms, opposite finger motion.
    if (Math.abs(velocity) < .02) return;
    const fling = { start: last.time, offset: panelOffset, velocity, frame: null };
    panelFling = fling;
    const step = now => {
      // Cancellation revokes the object before a queued callback can apply a frame.
      if (panelFling !== fling) return;
      if (!canScroll(gesture) || !panelRange()) { stopPanelFling(); return; }
      const elapsed = Math.max(0, now - fling.start), decay = Math.exp(-elapsed / 250);
      movePanel(fling.offset + fling.velocity * 250 * (1 - decay));
      if (panelOffset <= 0 || panelOffset >= panelRange() || Math.abs(fling.velocity * decay) < .02) { stopPanelFling(); return; }
      fling.frame = requestAnimationFrame(step);
    };
    fling.frame = requestAnimationFrame(step);
  };
  host.addEventListener('pointerdown', event => {
    stopPanelFling();
    if (event.pointerType !== 'touch') { suppressTouchClick = false; return; }
    if (event.isPrimary === false || touch) { cancelTouchScroll(); return; }
    suppressTouchClick = false;
    touchSelection = null;
    if (!inputEnabled || !pointerEnabled || snapshotUpdating || imeAnimating || fontGesture || recoveryScreen || draggingBoundary) return;
    touch = { id: event.pointerId, x: event.clientX, y: event.clientY, lastY: event.clientY,
      remainder: 0, dragging: false, selected: pane === active, target: inputTarget, panel: panelRange() > 0, offset: panelOffset, samples: [] };
    samplePanel(touch, event);
  }, true);
  host.addEventListener('pointermove', event => {
    const gesture = touch;
    if (!gesture || event.pointerId !== gesture.id) return;
    samplePanel(gesture, event);
    const dx = event.clientX - gesture.x, dy = event.clientY - gesture.y;
    if (!gesture.dragging) {
      if (Math.abs(dx) >= 8 && Math.abs(dx) >= Math.abs(dy)) {
        // Let the browser pan the wider remote grid horizontally.
        api.cancelTouchScroll(); return;
      }
      if (Math.abs(dy) < 8 || Math.abs(dy) <= Math.abs(dx)) return;
      gesture.dragging = true; suppressTouchClick = true;
      if (gesture.panel) panelDragging = true;
      if (event.isTrusted) host.setPointerCapture?.(gesture.id);
      if (!gesture.selected) {
        touchSelection = { pane, target: gesture.target };
        send({ type: 'focus', pane, target: gesture.target });
      }
    }
    event.preventDefault();
    if (!gesture.selected) return; // The first drag only selects this pane.
    if (!canScroll(gesture)) { api.cancelTouchScroll(); return; }
    if (gesture.panel) {
      // The entire original surface moves one CSS pixel per finger pixel.
      movePanel(gesture.offset + gesture.y - event.clientY);
      return;
    }
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
    const previous = touch; touch = null; panelDragging = false;
    if (previous.dragging && previous.panel && previous.selected) flingPanel(previous, event);
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
    if (suppressTouchClick || panelFling) { event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
  host.addEventListener('wheel', event => {
    if (panelDragging || panelFling) { event.preventDefault(); event.stopImmediatePropagation(); }
  }, { capture: true, passive: false });
  host.addEventListener('click', () => {
    if (!inputEnabled || fontGesture) return;
    if (pane !== active) send({ type: 'focus', pane, target: inputTarget });
    api.focusTerminal();
  });
  return Object.assign(api, { term, host });
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
  window.setInputEnabled = value => { enabled = value === true; if (!enabled) cancelTouchScroll(); for (const p of panes.values()) p.setInputEnabled(enabled); };
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
  let imeAnimating = false, imeVersion = 0;
  let cell = { width: 8.54, height: 19.6 }, boundaries = [], draggingBoundary = false;
  function readPaneContent(term) {
    const buffer = term.buffer?.active;
    if (!buffer) return null;
    const fullscreen = buffer.type === 'alternate' || (term.modes?.mouseTrackingMode && term.modes.mouseTrackingMode !== 'none');
    if (fullscreen) return { rows: term.rows, fullscreen: true, safe: false };
    if (buffer.type !== 'normal' || typeof buffer.getLine !== 'function' || !Number.isInteger(buffer.cursorY)) return null;
    let rows = 0;
    for (let y = term.rows - 1; y >= 0; --y) {
      const line = buffer.getLine(buffer.viewportY + y);
      if (!line) return null;
      if (line.translateToString(true, 0, term.cols).trim().length) { rows = y + 1; break; }
    }
    const cursor = buffer.baseY + buffer.cursorY - buffer.viewportY;
    if (cursor >= 0 && cursor < term.rows) rows = Math.max(rows, cursor + 1);
    return { rows, fullscreen: false, safe: buffer.viewportY === buffer.baseY };
  }
  function contentGeometry() {
    const specs = layout?.window != null ? layout.panes.filter(p => p.visible) : [{ id: 'shell', top: 0 }];
    let bottom = 0, safe = specs.length > 0 && !recoveryScreen, fullscreen = false;
    for (const spec of specs) {
      const pane = panes.get(spec.id), content = pane?.content;
      const known = content && !pane.snapshotting();
      const rows = content?.rows ?? spec.height ?? pane?.term.rows ?? 0;
      bottom = Math.max(bottom, (spec.top + rows) * cell.height);
      safe &&= !!known && content.safe;
      fullscreen ||= !!known && content.fullscreen;
    }
    if (!safe && !bottom) bottom = logicalHeight();
    return { bottom, safe, fullscreen };
  }
  function updateKeyboardLayout() {
    const content = contentGeometry();
    if (!imeAnimating && !fontGesture) {
      if (!viewport?.imeVisible) {
        keyboardChoice = 'pan'; autoPanLocked = false; keyboardDecisionPending = true;
      } else if (keyboardMode !== 'auto') keyboardChoice = keyboardMode;
      else {
        if (keyboardChoice === 'resize' && content.fullscreen) {
          autoPanLocked = true; keyboardChoice = 'pan';
        }
        if (keyboardDecisionPending) {
          // Compare with complete visible cells, not the fractional space below them.
          const available = Math.floor(viewport.visibleHeight / cell.height) * cell.height;
          keyboardChoice = !autoPanLocked && content.safe && content.bottom <= available + .001 ? 'resize' : 'pan';
          keyboardDecisionPending = false;
        }
      }
    }
    const atBottom = Math.abs(panelOffset - panelLimit) < .001;
    panelHeight = content.bottom;
    panelLimit = viewport?.imeVisible && keyboardChoice !== 'resize' ? Math.max(0, panelHeight - viewport.visibleHeight) : 0;
    panelOffset = atBottom ? panelLimit : Math.min(panelOffset, panelLimit);
    alignViewport();
    return viewport?.imeVisible && keyboardChoice === 'resize' ? viewport.visibleHeight : logicalHeight();
  }
  function sizeCanvas() {
    if (!canvas.style || root.clientWidth <= 0 || root.clientHeight <= 0) return;
    const tmux = layout?.window != null;
    const shell = panes.get('shell').term;
    const width = (tmux ? layout.columns : shell.cols) * cell.width;
    const height = (tmux ? layout.rows : shell.rows) * cell.height;
    if (!(width > 0 && height > 0)) return;
    canvas.style.width = `${Math.max(logicalWidth(), width)}px`;
    canvas.style.height = `${Math.max(logicalHeight(), height)}px`;
    root.scrollLeft = Math.max(0, Math.min(root.scrollLeft || 0, Math.max(0, width - root.clientWidth)));
    root.scrollTop = viewport?.imeVisible ? 0 : Math.max(0, Math.min(root.scrollTop || 0, Math.max(0, height - root.clientHeight)));
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
  function movePanel(offset) {
    panelOffset = Math.max(0, Math.min(panelRange(), offset));
    alignViewport();
  }
  function alignViewport() {
    if (canvas.style) canvas.style.transform = panelRange() > 0 ? `translateY(${-panelOffset}px)` : '';
    if (!panelBar) return;
    const range = panelRange(), visible = viewport?.visibleHeight || 0;
    panelBar.style.display = range > 0 && !viewportDetached ? 'block' : 'none';
    panelBar.style.height = `${visible}px`;
    const thumb = range > 0 ? Math.min(visible, Math.max(24, visible * visible / panelHeight)) : visible;
    panelThumb.style.height = `${thumb}px`;
    panelThumb.style.top = `${range > 0 ? panelOffset / range * (visible - thumb) : 0}px`;
  }
  function ensurePanelBar() {
    if (panelBar) return;
    panelBar = document.createElement('div'); panelBar.className = 'panel-scrollbar';
    panelBar.setAttribute('aria-hidden', 'true');
    panelThumb = document.createElement('div'); panelThumb.className = 'panel-scrollbar-thumb';
    panelBar.appendChild(panelThumb); document.body.appendChild(panelBar);
    panelBar.addEventListener('pointerdown', event => {
      if (!panelRange() || !enabled || !pointerEnabled || fontGesture || imeAnimating || recoveryScreen || barDrag || event.isPrimary === false) return;
      cancelTouchScroll();
      barDrag = { id: event.pointerId, y: event.clientY, offset: panelOffset, target: inputTarget };
      panelDragging = true;
      if (event.isTrusted) panelBar.setPointerCapture?.(event.pointerId);
      event.preventDefault(); event.stopImmediatePropagation();
    });
    panelBar.addEventListener('pointermove', event => {
      if (!barDrag || event.pointerId !== barDrag.id) return;
      if (barDrag.target !== inputTarget || !panelRange()) { cancelTouchScroll(); return; }
      const visible = viewport.visibleHeight;
      const thumb = Math.min(visible, Math.max(24, visible * visible / panelHeight));
      const travel = visible - thumb;
      if (travel > 0) movePanel(barDrag.offset + (event.clientY - barDrag.y) * panelRange() / travel);
      event.preventDefault(); event.stopImmediatePropagation();
    });
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) panelBar.addEventListener(type, event => {
      if (barDrag?.id === event.pointerId) cancelTouchScroll();
      event.preventDefault(); event.stopImmediatePropagation();
    });
    for (const type of ['mousedown', 'mouseup', 'click']) panelBar.addEventListener(type, event => {
      event.preventDefault(); event.stopImmediatePropagation();
    }, true);
  }
  window.setViewport = value => {
    if (!value || !Number.isSafeInteger(value.version) || value.version <= viewportVersion ||
        ![value.width, value.logicalHeight, value.visibleHeight].every(n => Number.isFinite(n) && n > 0) ||
        value.visibleHeight > value.logicalHeight || typeof value.imeVisible !== 'boolean') return;
    cancelTouchScroll();
    const logicalChanged = viewportDetached || !viewport || viewport.width !== value.width || viewport.logicalHeight !== value.logicalHeight;
    if (logicalChanged || viewport?.imeVisible !== value.imeVisible || viewport?.visibleHeight !== value.visibleHeight) keyboardDecisionPending = true;
    if (!viewport?.imeVisible && value.imeVisible) autoPanLocked = false;
    viewportVersion = value.version; viewportDetached = false; viewport = { ...value };
    ensurePanelBar();
    if (logicalChanged) resize();
    else {
      const height = updateKeyboardLayout();
      if (!imeAnimating && height !== sizingHeight) resize();
    }
  };
  window.detachViewport = version => {
    if (!Number.isSafeInteger(version) || version <= viewportVersion) return;
    viewportVersion = version; viewportDetached = true;
    keyboardDecisionPending = true; autoPanLocked = false;
    cancelTouchScroll(); alignViewport();
  };
  window.setImeAnimation = (animating, version) => {
    if (!Number.isSafeInteger(version) || version < imeVersion) return;
    imeVersion = version; imeAnimating = animating === true;
    cancelTouchScroll();
    if (!imeAnimating) requestAnimationFrame(() => { if (version === imeVersion && !imeAnimating) resize(); });
  };
  window.refreshViewport = () => resize();
  function resize() {
    if (viewportDetached || (nativeViewport && !viewport)) return;
    const term = focused().term;
    const rect = term.element?.querySelector('.xterm-screen')?.getBoundingClientRect();
    if (!rect?.width || !rect?.height || !term.cols || !term.rows || root.clientWidth <= 0 || root.clientHeight <= 0) return;
    const measured = { width: rect.width / term.cols, height: rect.height / term.rows };
    const changed = measured.width !== cell.width || measured.height !== cell.height;
    cell = measured;
    const columns = Math.max(2, Math.min(500, Math.floor(logicalWidth() / cell.width)));
    const requestedHeight = updateKeyboardLayout();
    const height = imeAnimating ? sizingHeight ?? logicalHeight() : requestedHeight;
    const rows = Math.max(2, Math.min(250, Math.floor(height / cell.height)));
    const key = `${columns},${rows}`;
    alignViewport();
    if (fontGesture) {
      if (changed) arrange(); else sizeCanvas();
      return;
    }
    if (!imeAnimating) sizingHeight = height;
    if (key !== lastViewport) { lastViewport = key; send({ type: 'resize', columns, rows }); arrange(); }
    else if (changed) arrange();
    else sizeCanvas();
  }

  window.configure = (font, history, mode = keyboardMode) => {
    const nextMode = ['auto', 'pan', 'resize'].includes(mode) ? mode : 'auto';
    if (nextMode !== keyboardMode) {
      keyboardMode = nextMode; keyboardDecisionPending = true; autoPanLocked = false;
      cancelTouchScroll();
    }
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
    updateKeyboardLayout();
    sizeCanvas();
    if (!layout) return;
    const tmux = layout.window !== null;
    shell.host.style.visibility = tmux ? 'hidden' : 'visible';
    boundaries.forEach(b => b.remove()); boundaries = [];
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
    // Native identity excludes the size generation, so our own resize cannot
    // restart auto selection when the server publishes its updated layout.
    const destinationChanged = layout?.window !== value.window || active !== (value.active || 'shell') ||
      (layout?.inputIdentity ?? layout?.inputTarget) !== (value.inputIdentity ?? value.inputTarget);
    if (destinationChanged) { keyboardDecisionPending = true; autoPanLocked = false; }
    const selectedByTouch = touchSelection !== null && touchSelection.pane === value.active && touchSelection.target === inputTarget;
    const geometryChanged = !layout || layout.columns !== value.columns || layout.rows !== value.rows ||
      layout.panes.length !== value.panes.length || value.panes.some((p, i) =>
        ['id', 'left', 'top', 'width', 'height', 'visible'].some(key => p[key] !== layout.panes[i][key]));
    if (geometryChanged || active !== (value.active || 'shell') || inputTarget !== value.inputTarget || layout?.window !== value.window) {
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
    if (destinationChanged && viewport?.imeVisible) resize();
    if (enabled && document.hasFocus() && ownedFocus && previous !== active && !selectedByTouch) focused().focusTerminal();
  };
  // The server may keep a smaller grid than this viewport. Tapping the empty
  // container still opens the active pane's IME, just as tapping its cells does.
  root.addEventListener('pointerdown', stopPanelFling, true);
  root.addEventListener('scroll', () => { if (viewport?.imeVisible && root.scrollTop) root.scrollTop = 0; });
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
