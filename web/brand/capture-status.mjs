import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const brandDir = __dirname;
const artifactDir = process.env.ARTIFACT_DIR || null;

// Vector mark SVG paths for inline embedding
function getMarkSvg(isDark) {
  const tGrad1Start = isDark ? '#F1F5F9' : '#0F172A';
  const tGrad1End = isDark ? '#94A3B8' : '#334155';
  const tGrad2End = isDark ? '#64748B' : '#475569';
  const strokeColor = isDark ? '#334155' : '#CBD5E1';
  const glowOpacity = isDark ? '0.2' : '0.1';

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="106 106 300 300" fill="none" role="img" aria-label="Sotto Mark" class="sotto-mark-svg">
  <defs>
    <linearGradient id="tUpper_${isDark ? 'd' : 'l'}" x1="120" y1="90" x2="380" y2="300" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="${tGrad1Start}" />
      <stop offset="60%" stop-color="${tGrad1End}" />
      <stop offset="100%" stop-color="${tGrad2End}" />
    </linearGradient>
    <linearGradient id="tLower_${isDark ? 'd' : 'l'}" x1="392" y1="422" x2="132" y2="212" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="${tGrad1Start}" />
      <stop offset="60%" stop-color="${tGrad1End}" />
      <stop offset="100%" stop-color="${tGrad2End}" />
    </linearGradient>
    <linearGradient id="cyanCore_${isDark ? 'd' : 'l'}" x1="220" y1="220" x2="292" y2="292" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="#67E8F9" />
      <stop offset="50%" stop-color="#38BDF8" />
      <stop offset="100%" stop-color="#06B6D4" />
    </linearGradient>
    <filter id="coreGlow_${isDark ? 'd' : 'l'}" x="180" y="180" width="152" height="152" filterUnits="userSpaceOnUse">
      <feGaussianBlur stdDeviation="10" result="blur" />
      <feComposite in="SourceGraphic" in2="blur" operator="over" />
    </filter>
  </defs>
  <circle cx="256" cy="256" r="36" fill="#38BDF8" opacity="${glowOpacity}" filter="url(#coreGlow_${isDark ? 'd' : 'l'})" />
  <path d="M 376 156 L 336 108 L 204 108 C 152 108 116 144 116 196 C 116 236 140 264 176 284 L 242 320 L 268 304 L 220 274 C 192 258 178 240 178 214 C 178 188 198 168 228 168 L 326 168 L 376 156 Z" fill="url(#tUpper_${isDark ? 'd' : 'l'})" stroke="${strokeColor}" stroke-width="2" stroke-linejoin="round" />
  <path d="M 136 356 L 176 404 L 308 404 C 360 404 396 368 396 316 C 396 276 372 248 336 228 L 270 192 L 244 208 L 292 238 C 320 254 334 272 334 298 C 334 324 314 344 284 344 L 186 344 L 136 356 Z" fill="url(#tLower_${isDark ? 'd' : 'l'})" stroke="${strokeColor}" stroke-width="2" stroke-linejoin="round" />
  <circle cx="256" cy="256" r="14" fill="url(#cyanCore_${isDark ? 'd' : 'l'})" stroke="${isDark ? '#083344' : '#E0F2FE'}" stroke-width="2" />
  <circle cx="256" cy="256" r="6" fill="#FFFFFF" />
</svg>`;
}

// Generates 90 vertical bars matching Anthropic's status page layout
function render90Bars({ pattern = 'pure-green' } = {}) {
  let bars = '';
  for (let i = 89; i >= 0; i--) {
    let colorClass = 'bar-ok';
    let tooltip = `Day ${i}: 100% Operational`;

    if (pattern === 'single-maint' && i === 24) {
      colorClass = 'bar-maint';
      tooltip = `Day ${i}: Scheduled Maintenance (Completed)`;
    } else if (pattern === 'slight-degraded' && i === 18) {
      colorClass = 'bar-warn';
      tooltip = `Day ${i}: Brief latency convergence (Resolved)`;
    } else if (pattern === 'realistic') {
      if (i === 12 || i === 48) {
        colorClass = 'bar-warn';
        tooltip = `Day ${i}: 99.8% Latency deviation`;
      } else if (i === 65) {
        colorClass = 'bar-maint';
        tooltip = `Day ${i}: Scheduled edge cert rotation`;
      }
    }

    bars += `<div class="bar ${colorClass}" role="img" aria-label="${tooltip}" tabindex="0" title="${tooltip}"></div>`;
  }
  return bars;
}

function buildStatusPageHtml(isDark) {
  const markSvg = getMarkSvg(isDark);

  const services = [
    {
      name: 'API & Ingestion',
      status: 'Operational',
      statusClass: 'status-ok',
      uptime: '100 % uptime',
      pattern: 'pure-green',
      summary: 'API & Ingestion is 100% operational with no recorded incidents over the past 90 days.',
    },
    {
      name: 'Web Dashboard & Decryption',
      status: 'Operational',
      statusClass: 'status-ok',
      uptime: '99.98 % uptime',
      pattern: 'single-maint',
      summary: 'Web Dashboard & Decryption is operational. 99.98% historical uptime with 1 scheduled maintenance window completed on day 24.',
    },
    {
      name: 'Cloud KMS & Zero-Knowledge Enclave',
      status: 'Operational',
      statusClass: 'status-ok',
      uptime: '100 % uptime',
      pattern: 'pure-green',
      summary: 'Cloud KMS & Zero-Knowledge Enclave is 100% operational with continuous zero-knowledge cryptographic verification.',
    },
    {
      name: 'Database Replica & Backup Engine',
      status: 'Operational',
      statusClass: 'status-ok',
      uptime: '99.95 % uptime',
      pattern: 'slight-degraded',
      summary: 'Database Replica & Backup Engine is operational. 99.95% historical uptime with a brief latency convergence resolved on day 18.',
    },
    {
      name: 'Webhooks & Stripe Billing',
      status: 'Operational',
      statusClass: 'status-ok',
      uptime: '100 % uptime',
      pattern: 'pure-green',
      summary: 'Webhooks & Stripe Billing is 100% operational with verified idempotent ledger processing.',
    },
  ];

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sotto Status</title>
  <style>
    :root {
      ${isDark ? `
      --bg:             #0b0f17;
      --card-bg:        #101623;
      --card-border:    #1e293b;
      --divider:        #1a2436;
      --ink:            #f8fafc;
      --ink-dim:        #94a3b8;
      --ink-meta:       #64748b;
      --ok:             #22c55e;
      --ok-banner-bg:   #15803d;
      --ok-banner-text: #ffffff;
      --warn:           #f59e0b;
      --maint:          #38bdf8;
      --rule:           #243247;
      ` : `
      --bg:             #f8fafc;
      --card-bg:        #ffffff;
      --card-border:    #e2e8f0;
      --divider:        #edf2f7;
      --ink:            #0f172a;
      --ink-dim:        #475569;
      --ink-meta:       #64748b;
      --ok:             #15803d;
      --ok-banner-bg:   #15803d;
      --ok-banner-text: #ffffff;
      --warn:           #d97706;
      --maint:          #0284c7;
      --rule:           #e2e8f0;
      `}
      --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      --font-mono: ui-monospace, "SF Mono", "Cascadia Code", Menlo, monospace;
    }

    * { margin: 0; padding: 0; box-sizing: border-box; }

    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }

    body {
      background: var(--bg);
      color: var(--ink);
      font-family: var(--font-sans);
      font-size: 15px;
      line-height: 1.5;
      padding-bottom: 80px;
      -webkit-font-smoothing: antialiased;
    }

    .container {
      max-width: 1040px;
      margin: 0 auto;
      padding: 0 28px;
    }

    /* Header & Nav Bar (Kept exactly as requested) */
    header {
      padding: 24px 0 20px;
      border-bottom: 1px solid var(--card-border);
      background: var(--bg);
    }

    .header-inner {
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 16px;
    }

    .wordmark-wrap {
      display: inline-flex;
      align-items: center;
      gap: 9px;
      text-decoration: none;
      color: inherit;
    }

    .sotto-mark-svg {
      width: 0.88em;
      height: 0.88em;
      display: inline-block;
      flex-shrink: 0;
    }

    .brand-title {
      font-size: 1.25rem;
      font-weight: 700;
      letter-spacing: -0.03em;
      color: var(--ink);
      line-height: 1;
    }

    .sub-domain {
      font-family: var(--font-mono);
      font-size: 0.8125rem;
      color: var(--ink-meta);
      margin-left: 2px;
    }

    nav {
      font-family: var(--font-mono);
      font-size: 0.8125rem;
      display: flex;
      flex-wrap: wrap;
      gap: 16px;
    }

    nav a {
      color: var(--ink-dim);
      text-decoration: none;
      transition: color 0.15s;
    }
    nav a:hover {
      color: var(--ink);
    }

    /* Clean Overall Status Banner - Green Outline (No Fill) */
    .status-banner {
      margin-top: 36px;
      margin-bottom: 24px;
      background: transparent;
      border: 1px solid var(--ok);
      color: var(--ok);
      padding: 14px 22px;
      border-radius: 4px;
      font-size: 1rem;
      font-weight: 600;
      letter-spacing: -0.01em;
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 12px;
    }

    .status-banner-left {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .status-banner-dot {
      width: 7px;
      height: 7px;
      background: var(--ok);
      border-radius: 50%;
      display: inline-block;
    }

    .status-banner-time {
      font-size: 0.8125rem;
      font-weight: 400;
      color: var(--ink-meta);
      font-family: var(--font-mono);
    }

    /* Simple Status Showcase Card (Matching Anthropic Screenshot) */
    .status-card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 5px;
      overflow: hidden;
    }

    .service-row {
      padding: 24px 28px;
      border-bottom: 1px solid var(--divider);
    }
    .service-row:last-child {
      border-bottom: none;
    }

    .service-head {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 8px;
      margin-bottom: 14px;
    }

    .service-name {
      font-size: 1.0625rem;
      font-weight: 700;
      color: var(--ink);
      letter-spacing: -0.01em;
    }

    .service-status {
      font-size: 0.9375rem;
      font-weight: 500;
    }
    .status-ok {
      color: var(--ok);
    }
    .status-warn {
      color: var(--warn);
    }

    /* 90 Days Vertical Bars */
    .bars-container {
      display: flex;
      gap: 3px;
      height: 34px;
      margin-bottom: 12px;
    }

    .bar {
      flex: 1;
      height: 100%;
      border-radius: 2px;
      transition: opacity 0.1s, transform 0.1s;
    }
    .bar:hover {
      opacity: 0.8;
      transform: scaleY(1.08);
    }

    .bar-ok {
      background: var(--ok);
    }
    .bar-warn {
      background: var(--warn);
    }
    .bar-maint {
      background: var(--maint);
    }

    /* Legend Row with Horizontal Lines (Exact screenshot layout) */
    .legend-row {
      display: flex;
      align-items: center;
      gap: 14px;
      font-size: 0.8125rem;
      color: var(--ink-meta);
      font-family: var(--font-sans);
      flex-wrap: wrap;
    }

    .legend-line {
      flex: 1;
      height: 1px;
      background: var(--rule);
    }

    .uptime-label {
      color: var(--ink-dim);
      font-weight: 500;
      white-space: nowrap;
    }

    @media (max-width: 640px) {
      .container {
        padding: 0 16px;
      }
      .service-row {
        padding: 18px 16px;
      }
      .bars-container {
        gap: 2px;
        height: 28px;
      }
      .bar {
        min-width: 1px;
      }
      .legend-line {
        min-width: 8px;
      }
      footer {
        flex-direction: column;
        gap: 12px;
      }
    }

    /* Footer */
    footer {
      margin-top: 48px;
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      font-size: 0.8125rem;
      color: var(--ink-meta);
    }

    footer a {
      color: var(--ink-dim);
      text-decoration: none;
    }
    footer a:hover {
      color: var(--ink);
    }
  </style>
</head>
<body>

  <header>
    <div class="container header-inner">
      <a href="https://getsotto.co.uk" class="wordmark-wrap">
        ${markSvg}
        <span class="brand-title">Sotto</span>
        <span class="sub-domain">// status</span>
      </a>

      <nav>
        <a href="#services">Services</a>
        <a href="https://github.com/getsotto/sotto">GitHub</a>
        <a href="https://getsotto.co.uk">getsotto.co.uk →</a>
      </nav>
    </div>
  </header>

  <main class="container">
    <!-- Overall Status Banner (Green Outline) -->
    <div class="status-banner">
      <div class="status-banner-left">
        <span class="status-banner-dot"></span>
        <span>All Systems Operational</span>
      </div>
      <span class="status-banner-time">Refreshed 1m ago</span>
    </div>

    <!-- Status Showcase (Direct match to user screenshot) -->
    <div class="status-card" id="services">
      ${services
        .map(
          (s) => `
      <div class="service-row">
        <div class="service-head">
          <span class="service-name">${s.name}</span>
          <span class="service-status ${s.statusClass}">${s.status}</span>
        </div>
        <div class="sr-only">${s.summary}</div>

        <div class="bars-container" role="group" aria-label="90-day uptime history for ${s.name}">
          ${render90Bars({ pattern: s.pattern })}
        </div>

        <div class="legend-row">
          <span>90 days ago</span>
          <div class="legend-line"></div>
          <span class="uptime-label">${s.uptime}</span>
          <div class="legend-line"></div>
          <span>Today</span>
        </div>
      </div>`
        )
        .join('')}
    </div>

    <footer>
      <div>Sotto Zero-Knowledge Secret Sync · <a href="https://getsotto.co.uk">getsotto.co.uk</a></div>
      <div>
        <a href="https://github.com/getsotto/sotto/blob/main/SECURITY.md">Security Policy</a> ·
        <a href="https://github.com/getsotto/sotto/blob/main/THREAT-MODEL.md">Threat Model</a>
      </div>
    </footer>
  </main>

</body>
</html>`;
}

function buildBrowserMockupHtml({ isDark, heroBase64 }) {
  const bg = isDark ? '#050811' : '#E2E8F0';
  const frameBg = isDark ? '#0B0F17' : '#F8FAFC';
  const border = isDark ? '#1E293B' : '#CBD5E1';
  const titleBarBg = isDark ? '#101623' : '#F1F5F9';
  const titleBorder = isDark ? '#1E293B' : '#E2E8F0';
  const addressBg = isDark ? '#0B0F17' : '#FFFFFF';
  const addressBorder = isDark ? '#1E293B' : '#CBD5E1';
  const lockColor = isDark ? '#38BDF8' : '#0284C7';
  const urlColor = isDark ? '#F8FAFC' : '#0F172A';
  const urlMuted = isDark ? '#64748B' : '#94A3B8';
  const shadow = isDark
    ? '0 25px 60px rgba(0,0,0,0.85), 0 0 30px rgba(6, 182, 212, 0.08)'
    : '0 25px 60px rgba(15, 23, 42, 0.18), 0 0 20px rgba(2, 132, 199, 0.06)';

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
      border-radius: 8px;
      overflow: hidden;
      box-shadow: ${shadow};
      display: flex;
      flex-direction: column;
    }
    .title-bar {
      height: 46px;
      background: ${titleBarBg};
      border-bottom: 1px solid ${titleBorder};
      display: flex;
      align-items: center;
      padding: 0 16px;
      gap: 16px;
    }
    .traffic-lights {
      display: flex;
      gap: 8px;
    }
    .dot {
      width: 11px;
      height: 11px;
      border-radius: 50%;
    }
    .dot.red { background: #EF4444; }
    .dot.yellow { background: #F59E0B; }
    .dot.green { background: #10B981; }
    .address-bar {
      flex: 1;
      max-width: 620px;
      margin: 0 auto;
      height: 28px;
      background: ${addressBg};
      border: 1px solid ${addressBorder};
      border-radius: 4px;
      display: flex;
      align-items: center;
      padding: 0 12px;
      gap: 8px;
      font-family: ui-monospace, "SF Mono", monospace;
      font-size: 13px;
    }
    .lock-icon {
      color: ${lockColor};
      font-size: 11px;
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
        <span class="url">https://status.getsotto.co.uk</span>
        <span class="url-muted">/</span>
      </div>
    </div>
    <div class="viewport">
      <img src="data:image/png;base64,${heroBase64}" alt="status.getsotto.co.uk viewport" />
    </div>
  </div>
</body>
</html>`;
}

async function run() {
  console.log('Rendering and capturing clean, simple status.getsotto.co.uk mockups...');
  const browser = await chromium.launch();

  // Save standalone status HTML pages
  const htmlDark = buildStatusPageHtml(true);
  const htmlLight = buildStatusPageHtml(false);
  fs.writeFileSync(path.join(brandDir, 'status-page-dark.html'), htmlDark);
  fs.writeFileSync(path.join(brandDir, 'status-page-light.html'), htmlLight);

  // 1. Dark Vault Mode
  console.log('Capturing Dark Vault status page...');
  const pageDark = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  });
  await pageDark.setContent(htmlDark, { waitUntil: 'networkidle' });

  const heroDarkBuffer = await pageDark.screenshot({ clip: { x: 0, y: 0, width: 1440, height: 1000 } });
  fs.writeFileSync(path.join(brandDir, 'status-hero.png'), heroDarkBuffer);

  const fullDarkBuffer = await pageDark.screenshot({ fullPage: true });
  fs.writeFileSync(path.join(brandDir, 'status-fullpage.png'), fullDarkBuffer);

  const mockupDarkHtml = buildBrowserMockupHtml({ isDark: true, heroBase64: heroDarkBuffer.toString('base64') });
  const mockupDarkPage = await browser.newPage({ viewport: { width: 1600, height: 1120 }, deviceScaleFactor: 2 });
  await mockupDarkPage.setContent(mockupDarkHtml);
  const mockupDarkBuffer = await mockupDarkPage.screenshot();
  fs.writeFileSync(path.join(brandDir, 'status-browser-mockup.png'), mockupDarkBuffer);
  console.log('✓ Captured Dark Vault status mockup');

  // 2. Machined Light Mode
  console.log('Capturing Machined Light status page...');
  const pageLight = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
    colorScheme: 'light',
  });
  await pageLight.setContent(htmlLight, { waitUntil: 'networkidle' });

  const heroLightBuffer = await pageLight.screenshot({ clip: { x: 0, y: 0, width: 1440, height: 1000 } });
  fs.writeFileSync(path.join(brandDir, 'status-hero-light.png'), heroLightBuffer);

  const fullLightBuffer = await pageLight.screenshot({ fullPage: true });
  fs.writeFileSync(path.join(brandDir, 'status-fullpage-light.png'), fullLightBuffer);

  const mockupLightHtml = buildBrowserMockupHtml({ isDark: false, heroBase64: heroLightBuffer.toString('base64') });
  const mockupLightPage = await browser.newPage({ viewport: { width: 1600, height: 1120 }, deviceScaleFactor: 2 });
  await mockupLightPage.setContent(mockupLightHtml);
  const mockupLightBuffer = await mockupLightPage.screenshot();
  fs.writeFileSync(path.join(brandDir, 'status-browser-mockup-light.png'), mockupLightBuffer);
  console.log('✓ Captured Machined Light status mockup');

  // 3. Copy all to Artifact directory (optional)
  if (artifactDir) {
    fs.mkdirSync(artifactDir, { recursive: true });
    fs.copyFileSync(path.join(brandDir, 'status-hero.png'), path.join(artifactDir, 'status-hero.png'));
    fs.copyFileSync(path.join(brandDir, 'status-fullpage.png'), path.join(artifactDir, 'status-fullpage.png'));
    fs.copyFileSync(path.join(brandDir, 'status-browser-mockup.png'), path.join(artifactDir, 'status-browser-mockup.png'));
    fs.copyFileSync(path.join(brandDir, 'status-hero-light.png'), path.join(artifactDir, 'status-hero-light.png'));
    fs.copyFileSync(path.join(brandDir, 'status-fullpage-light.png'), path.join(artifactDir, 'status-fullpage-light.png'));
    fs.copyFileSync(path.join(brandDir, 'status-browser-mockup-light.png'), path.join(artifactDir, 'status-browser-mockup-light.png'));
    console.log('✓ Synchronized all status mockups with artifact directory');
  }

  await browser.close();
  console.log('All status page captures completed!');
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
