import { createReadStream } from 'node:fs';
import { readFile, stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const outputRoot = resolve(projectRoot, 'out');
const siteConfig = JSON.parse(
  await readFile(resolve(projectRoot, 'rusty-docs-site.json'), 'utf8'),
);
const basePath =
  siteConfig.basePath === '/'
    ? ''
    : `/${siteConfig.basePath.split('/').filter(Boolean).join('/')}`;
const contentTypes = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.jpeg': 'image/jpeg',
  '.jpg': 'image/jpeg',
  '.md': 'text/markdown; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.txt': 'text/plain; charset=utf-8',
  '.webp': 'image/webp',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
};

function sendNotFound(response) {
  response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
  response.end('Not found');
}

const server = createServer(async (request, response) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { allow: 'GET, HEAD' });
    response.end();
    return;
  }

  let pathname;
  try {
    pathname = decodeURIComponent(
      new URL(request.url ?? '/', 'http://localhost').pathname,
    );
  } catch {
    sendNotFound(response);
    return;
  }

  if (basePath && (pathname === '/' || pathname === basePath)) {
    response.writeHead(302, { location: `${basePath}/` });
    response.end();
    return;
  }
  if (basePath && !pathname.startsWith(`${basePath}/`)) {
    sendNotFound(response);
    return;
  }

  const sitePath = basePath ? pathname.slice(basePath.length) || '/' : pathname;
  const requestedPath = resolve(outputRoot, `.${sitePath}`);
  const candidates = [
    requestedPath,
    resolve(requestedPath, 'index.html'),
    `${requestedPath}.html`,
  ];
  let filePath;
  let fileInfo;
  for (const candidate of candidates) {
    if (candidate !== outputRoot && !candidate.startsWith(`${outputRoot}${sep}`)) {
      continue;
    }
    try {
      const candidateInfo = await stat(candidate);
      if (candidateInfo.isFile()) {
        filePath = candidate;
        fileInfo = candidateInfo;
        break;
      }
      if (candidateInfo.isDirectory()) {
        const indexPath = resolve(candidate, 'index.html');
        const indexInfo = await stat(indexPath);
        if (indexInfo.isFile()) {
          filePath = indexPath;
          fileInfo = indexInfo;
          break;
        }
      }
    } catch {
      continue;
    }
  }
  if (!filePath || !fileInfo) {
    sendNotFound(response);
    return;
  }

  response.writeHead(200, {
    'content-length': fileInfo.size,
    'content-type':
      contentTypes[extname(filePath)] ?? 'application/octet-stream',
    'x-content-type-options': 'nosniff',
  });
  if (request.method === 'HEAD') {
    response.end();
    return;
  }

  const stream = createReadStream(filePath);
  stream.on('error', () => response.destroy());
  stream.pipe(response);
});

const port = Number(process.env.PORT ?? 3000);
server.listen(port, '0.0.0.0', () => {
  console.log(`Preview URL: http://localhost:${port}${basePath}/`);
});
