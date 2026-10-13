import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const brandDir = __dirname;
const distDir = path.resolve(__dirname, '../dist');
const artifactDir = process.env.ARTIFACT_DIR || null;

const MIME = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.json': 'application/json',
  '.txt': 'text/plain',
  '.webmanifest': 'application/manifest+json',
};

function startServer(port = 4321) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      let reqPath = req.url.split('?')[0];
      if (reqPath === '/') reqPath = '/index.html';
      const filePath = path.join(distDir, reqPath);

      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        const ext = path.extname(filePath);
        res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' });
        fs.createReadStream(filePath).pipe(res);
      } else {
        const fallback = path.join(distDir, 'index.html');
        res.writeHead(200, { 'Content-Type': 'text/html' });
        fs.createReadStream(fallback).pipe(res);
      }
    });

    server.listen(port, () => resolve(server));
  });
}

function buildMockupHtml({ isDark, heroBase64 }) {
  const bg = isDark ? '#050811' : '#E2E8F0';
  const frameBg = isDark ? '#0B0F17' : '#F8FAFC';
  const border = isDark ? '#1E293B' : '#CBD5E1';
  const titleBarBg = isDark ? '#0F172A' : '#F1F5F9';
  const titleBorder = isDark ? '#1E293B' : '#E2E8F0';
  const addressBg = isDark ? '#0B0F17' : '#FFFFFF';
  const addressBorder = isDark ? '#1E293B' : '#CBD5E1';
  const lockColor = isDark ? '#06B6D4' : '#0284C7';
  const urlColor = isDark ? '#F8FAFC' : '#0F172A';
  const urlMuted = isDark ? '#64748B' : '#94A3B8';
  const shadow = isDark
    ? '0 25px 60px rgba(0,0,0,0.8), 0 0 40px rgba(6, 182, 212, 0.12)'
    : '0 25px 60px rgba(15, 23, 42, 0.18), 0 0 30px rgba(2, 132, 199, 0.08)';

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      width: 1600px;
      height: 1120px;
      background: ${bg};
      padding: 40px;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
    }
    .browser-frame {
      width: 1520px;
      height: 1040px;
      background: ${frameBg};
      border: 1px solid ${border};
      border-radius: 10px;
      overflow: hidden;
      box-shadow: ${shadow};
      display: flex;
      flex-direction: column;
    }
    .title-bar {
      height: 48px;
      background: ${titleBarBg};
      border-bottom: 1px solid ${titleBorder};
      display: flex;
      align-items: center;
      padding: 0 18px;
      gap: 16px;
    }
    .traffic-lights {
      display: flex;
      gap: 8px;
    }
    .dot {
      width: 12px;
      height: 12px;
      border-radius: 50%;
    }
    .dot.red { background: #EF4444; }
    .dot.yellow { background: #F59E0B; }
    .dot.green { background: #10B981; }
    .address-bar {
      flex: 1;
      max-width: 640px;
      margin: 0 auto;
      height: 30px;
      background: ${addressBg};
      border: 1px solid ${addressBorder};
      border-radius: 5px;
      display: flex;
      align-items: center;
      padding: 0 12px;
      gap: 8px;
      font-family: ui-monospace, "SF Mono", monospace;
      font-size: 13px;
    }
    .lock-icon {
      color: ${lockColor};
      font-size: 12px;
    }
    .url {
      color: ${urlColor};
      font-weight: 500;
    }
    .url-muted {
      color: ${urlMuted};
    }
    .viewport {
      flex: 1;
      width: 100%;
      overflow: hidden;
      position: relative;
    }
    .viewport img {
      width: 100%;
      height: auto;
      display: block;
    }
  </style>
</head>
<body>
  <div class="browser-frame">
    <div class="title-bar">
      <div class="traffic-lights">
        <div class="dot red"></div>
        <div class="dot yellow"></div>
        <div class="dot green"></div>
      </div>
      <div class="address-bar">
        <span class="lock-icon">🔒</span>
        <span class="url">https://getsotto.co.uk</span>
        <span class="url-muted">/</span>
      </div>
    </div>
    <div class="viewport">
      <img src="data:image/png;base64,${heroBase64}" alt="getsotto.co.uk viewport" />
    </div>
  </div>
</body>
</html>`;
}

async function capture() {
  console.log('Starting internal static preview server...');
  const server = await startServer(4321);
  const browser = await chromium.launch();

  // 1. Dark Vault Theme
  console.log('Capturing getsotto.co.uk dark mode...');
  const contextDark = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
    reducedMotion: 'reduce',
  });
  const pageDark = await contextDark.newPage();
  await pageDark.goto('http://127.0.0.1:4321/', { waitUntil: 'networkidle' });

  const heroDarkBuffer = await pageDark.screenshot({ clip: { x: 0, y: 0, width: 1440, height: 1000 } });
  fs.writeFileSync(path.join(brandDir, 'getsotto-hero.png'), heroDarkBuffer);

  const fullDarkBuffer = await pageDark.screenshot({ fullPage: true });
  fs.writeFileSync(path.join(brandDir, 'getsotto-fullpage.png'), fullDarkBuffer);

  const mockupDarkHtml = buildMockupHtml({ isDark: true, heroBase64: heroDarkBuffer.toString('base64') });
  const mockupDarkPage = await browser.newPage({ viewport: { width: 1600, height: 1120 }, deviceScaleFactor: 2 });
  await mockupDarkPage.setContent(mockupDarkHtml);
  const mockupDarkBuffer = await mockupDarkPage.screenshot();
  fs.writeFileSync(path.join(brandDir, 'getsotto-browser-mockup.png'), mockupDarkBuffer);
  console.log('✓ Captured dark mode previews');

  // 2. Machined Light Theme
  console.log('Capturing getsotto.co.uk light mode...');
  const contextLight = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
    colorScheme: 'light',
    reducedMotion: 'reduce',
  });
  const pageLight = await contextLight.newPage();
  await pageLight.goto('http://127.0.0.1:4321/', { waitUntil: 'networkidle' });

  const heroLightBuffer = await pageLight.screenshot({ clip: { x: 0, y: 0, width: 1440, height: 1000 } });
  fs.writeFileSync(path.join(brandDir, 'getsotto-hero-light.png'), heroLightBuffer);

  const fullLightBuffer = await pageLight.screenshot({ fullPage: true });
  fs.writeFileSync(path.join(brandDir, 'getsotto-fullpage-light.png'), fullLightBuffer);

  const mockupLightHtml = buildMockupHtml({ isDark: false, heroBase64: heroLightBuffer.toString('base64') });
  const mockupLightPage = await browser.newPage({ viewport: { width: 1600, height: 1120 }, deviceScaleFactor: 2 });
  await mockupLightPage.setContent(mockupLightHtml);
  const mockupLightBuffer = await mockupLightPage.screenshot();
  fs.writeFileSync(path.join(brandDir, 'getsotto-browser-mockup-light.png'), mockupLightBuffer);
  console.log('✓ Captured light mode previews');

  // 3. Copy all to artifact directory (optional)
  if (artifactDir) {
    fs.mkdirSync(artifactDir, { recursive: true });
    fs.copyFileSync(path.join(brandDir, 'getsotto-hero.png'), path.join(artifactDir, 'getsotto-hero.png'));
    fs.copyFileSync(path.join(brandDir, 'getsotto-fullpage.png'), path.join(artifactDir, 'getsotto-fullpage.png'));
    fs.copyFileSync(path.join(brandDir, 'getsotto-browser-mockup.png'), path.join(artifactDir, 'getsotto-browser-mockup.png'));
    fs.copyFileSync(path.join(brandDir, 'getsotto-hero-light.png'), path.join(artifactDir, 'getsotto-hero-light.png'));
    fs.copyFileSync(path.join(brandDir, 'getsotto-fullpage-light.png'), path.join(artifactDir, 'getsotto-fullpage-light.png'));
    fs.copyFileSync(path.join(brandDir, 'getsotto-browser-mockup-light.png'), path.join(artifactDir, 'getsotto-browser-mockup-light.png'));
    console.log('✓ Synchronized all previews with artifact directory');
  }

  await browser.close();
  server.close();
  console.log('All capture routines completed!');
}

capture().catch(err => {
  console.error(err);
  process.exit(1);
});
