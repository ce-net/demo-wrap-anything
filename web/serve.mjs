// node web/serve.mjs [port] [ws-door addr]: serve this page at /demos/wrap/ and pipe /wire to this machine's
// wire.ws-door (default 127.0.0.1:7792), the way the public edge does. No dependencies (node >= 18).
import http from 'node:http';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(fileURLToPath(import.meta.url));
const port = Number(process.argv[2] || 8977);
const [dh, dp] = (process.argv[3] || '127.0.0.1:7792').split(':');
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.json': 'application/json', '.css': 'text/css', '.woff2': 'font/woff2', '.png': 'image/png' };

const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p === '/' || p === '/demos/wrap') { res.writeHead(302, { location: '/demos/wrap/' }); return res.end(); }
  if (!p.startsWith('/demos/wrap/')) { res.writeHead(404); return res.end('not found'); }
  p = p.slice('/demos/wrap/'.length) || 'index.html';
  const f = path.join(root, path.normalize(p));
  if (!f.startsWith(root) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); return res.end('not found'); }
  res.writeHead(200, { 'content-type': types[path.extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
  fs.createReadStream(f).pipe(res);
});

// /wire: pass the upgrade request and every byte after it to the ws-door unchanged
server.on('upgrade', (req, sock, head) => {
  if (new URL(req.url, 'http://x').pathname !== '/wire') return sock.destroy();
  const up = net.connect(Number(dp), dh, () => {
    let h = `${req.method} ${req.url} HTTP/1.1\r\n`;
    for (let i = 0; i < req.rawHeaders.length; i += 2) h += `${req.rawHeaders[i]}: ${req.rawHeaders[i + 1]}\r\n`;
    up.write(h + '\r\n');
    if (head.length) up.write(head);
    up.pipe(sock).pipe(up);
  });
  up.on('error', () => sock.destroy());
  sock.on('error', () => up.destroy());
});

server.listen(port, '127.0.0.1', () => console.log(`http://127.0.0.1:${port}/demos/wrap/`));
