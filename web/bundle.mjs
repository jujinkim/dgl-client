import { copyFileSync, mkdirSync } from 'node:fs';
const target = new URL('../app/src/androidMain/assets/terminal/', import.meta.url);
mkdirSync(target, { recursive: true });
for (const [source, name] of [
  ['node_modules/@xterm/xterm/lib/xterm.js', 'xterm.js'],
  ['node_modules/@xterm/xterm/css/xterm.css', 'xterm.css'],
  ['node_modules/@xterm/xterm/LICENSE', 'LICENSE-xterm'],
]) copyFileSync(new URL(source, import.meta.url), new URL(name, target));
