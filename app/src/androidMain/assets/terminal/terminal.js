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
  // Remote escape sequences cannot access the clipboard.
  term.parser.registerOscHandler(52, () => true);
  term.onData(data => { if (new TextEncoder().encode(data).length <= 65536) post({ type: 'input', data }); });
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
  document.getElementById('terminal').addEventListener('click', () => term.focus());
  post({ type: 'ready' });
})();
