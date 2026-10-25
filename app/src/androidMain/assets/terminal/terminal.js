'use strict';
(() => {
  const post = message => window.dgl.postMessage(JSON.stringify(message));
  const term = new Terminal({
    cursorBlink: true, scrollback: 5000, fontSize: 14,
    fontFamily: 'monospace', allowProposedApi: false,
    theme: { background: '#11151b', foreground: '#dce1e8' },
    linkHandler: { activate() {}, hover() {}, leave() {} },
  });
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(document.getElementById('terminal'));
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
  window.setSmoothTransitions = enabled => {
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
  window.cancelSnapshot = clearCache;
  window.beginSnapshot = () => {
    const version = ++cacheVersion;
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
  window.endSnapshot = id => {
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
        const duration = reducedMotion ? 0 : smoothTransitions ? 240 : 180;
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
  window.setInputEnabled = enabled => { inputEnabled = enabled === true; };
  // Capture xterm's own paste transformation (including current bracketed-paste mode)
  // without emitting input. Kotlin validates the target and submits one acknowledged request.
  window.preparePaste = (text, enter) => {
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
  term.onResize(({ cols, rows }) => { if (!resizingRemote) post({ type: 'resize', columns: cols, rows }); });
  window.resizePane = (columns, rows) => {
    if (!Number.isInteger(columns) || !Number.isInteger(rows) || columns < 2 || columns > 500 || rows < 2 || rows > 250) return;
    resizingRemote = true;
    try { term.resize(columns, rows); } finally { resizingRemote = false; }
  };
  const resize = () => { try { fit.fit(); } catch {} };
  new ResizeObserver(resize).observe(document.getElementById('terminal'));
  window.configure = (fontSize, scrollback) => {
    if (Number.isInteger(fontSize) && fontSize >= 10 && fontSize <= 28) term.options.fontSize = fontSize;
    if (Number.isInteger(scrollback) && scrollback >= 100 && scrollback <= 10000) term.options.scrollback = scrollback;
    resize();
  };
  window.receive = (base64, reset, id) => {
    if (typeof base64 !== 'string' || base64.length > 22000 || !Number.isSafeInteger(id) || id <= 0) return;
    const data = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
    if (reset === true) term.reset();
    term.write(data, () => post({ type: 'ack', id }));
  };
  // xterm owns composition events; forwarding key/input events separately duplicates IME commits.
  // Focus after the WebView's default touch/mouse focus change. Doing this on
  // pointerdown lets the subsequent default click blur the textarea and hide IME.
  window.focusTerminal = () => term.focus();
  document.getElementById('terminal').addEventListener('click', window.focusTerminal);
  post({ type: 'ready' });
})();
