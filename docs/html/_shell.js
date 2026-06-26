/* Polyrocket UI — shared shell behavior v3
 * All topbar functions (theme / language / settings / command palette /
 * notifications) consolidated into the user-card popover at sidebar bottom.
 * Theme  : dark / light / matrix  (single brand voltage, no shadows)
 * Wallet : connected / disconnected (Polymarket mock)
 * Locale : zh / en                 (mock — toggles data-locale attr)
 */
(function () {
  const root = document.documentElement;
  const savedTheme = localStorage.getItem('polyrocket.theme') || 'dark';
  const savedLang  = localStorage.getItem('polyrocket.lang')  || 'zh';
  const savedWallet= localStorage.getItem('polyrocket.wallet')|| '';
  root.setAttribute('data-theme', savedTheme);
  root.setAttribute('data-locale', savedLang);

  // sidebar collapse
  const collapsed = localStorage.getItem('polyrocket.sidebar.collapsed') === '1';
  if (collapsed) root.setAttribute('data-sidebar', 'collapsed');
  document.addEventListener('click', (e) => {
    const t = e.target.closest('[data-toggle="sidebar"]');
    if (t) {
      const next = root.getAttribute('data-sidebar') === 'collapsed' ? '0' : '1';
      localStorage.setItem('polyrocket.sidebar.collapsed', next);
      root.setAttribute('data-sidebar', next === '1' ? 'collapsed' : '');
      location.reload();
    }
  });

  // active nav link
  const here = location.pathname.split('/').pop() || 'dashboard.html';
  function markActive() {
    document.querySelectorAll('[data-nav]').forEach((el) => {
      if (el.getAttribute('data-nav') === here) el.classList.add('active');
    });
  }
  markActive();

  // user card — toggle popover (binding happens after sidebar-foot rendered)
  function bindUserCard() {
    const userCard = document.querySelector('[data-user-card]');
    const userMenu = document.querySelector('[data-user-menu]');
    if (!userCard || !userMenu) return;
    function closeMenu() { userMenu.classList.remove('open'); }
    function openMenu()  { userMenu.classList.add('open'); }
    userCard.addEventListener('click', (e) => {
      e.stopPropagation();
      userMenu.classList.toggle('open');
    });
    userMenu.addEventListener('click', (e) => e.stopPropagation());
    document.addEventListener('click', closeMenu);
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeMenu(); });
    refreshChecks();
    markActive();
  }
  bindUserCard();

  // theme buttons
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-set-theme]');
    if (!btn) return;
    const t = btn.getAttribute('data-set-theme');
    localStorage.setItem('polyrocket.theme', t);
    root.setAttribute('data-theme', t);
    document.querySelectorAll('[data-set-theme]').forEach((b) => {
      b.classList.toggle('check', b.getAttribute('data-set-theme') === t);
      const val = b.querySelector('.val');
      if (val) val.textContent = b.getAttribute('data-set-theme') === t ? '✓' : '';
    });
  });

  // language buttons
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-set-lang]');
    if (!btn) return;
    const l = btn.getAttribute('data-set-lang');
    localStorage.setItem('polyrocket.lang', l);
    root.setAttribute('data-locale', l);
    document.querySelectorAll('[data-set-lang]').forEach((b) => {
      b.classList.toggle('check', b.getAttribute('data-set-lang') === l);
      const val = b.querySelector('.val');
      if (val) val.textContent = b.getAttribute('data-set-lang') === l ? '✓' : '';
    });
    // hook for translation re-render
    document.dispatchEvent(new CustomEvent('locale:change', { detail: l }));
  });

  // apply current theme/lang checks
  function refreshChecks() {
    document.querySelectorAll('[data-set-theme]').forEach((b) => {
      b.classList.toggle('check', b.getAttribute('data-set-theme') === savedTheme);
      const v = b.querySelector('.val');
      if (v) v.textContent = b.getAttribute('data-set-theme') === savedTheme ? '✓' : '';
    });
    document.querySelectorAll('[data-set-lang]').forEach((b) => {
      b.classList.toggle('check', b.getAttribute('data-set-lang') === savedLang);
      const v = b.querySelector('.val');
      if (v) v.textContent = b.getAttribute('data-set-lang') === savedLang ? '✓' : '';
    });
  }
  refreshChecks();

  // wallet — render user card or connect button based on localStorage
  const sidebarFoot = document.querySelector('.sidebar-foot');
  const wallet = localStorage.getItem('polyrocket.wallet') || '';
  if (sidebarFoot) {
    if (wallet) {
      const short = wallet.slice(0, 6) + '…' + wallet.slice(-4);
      const ens   = 'trader.eth';
      const initials = 'TR';
      sidebarFoot.innerHTML = `
        <div class="user-menu" data-user-menu>
          <div class="user-menu-section">账户</div>
          <div class="user-menu-item">
            <span class="ic">◉</span>
            <span class="lbl">${ens}</span>
            <span class="val">${short}</span>
          </div>
          <div class="user-menu-item">
            <span class="ic">$</span>
            <span class="lbl">余额</span>
            <span class="val">$2,840.12</span>
          </div>
          <div class="user-menu-item">
            <span class="ic">⚡</span>
            <span class="lbl">持仓</span>
            <span class="val">18 个市场</span>
          </div>
          <div class="user-menu-divider"></div>
          <div class="user-menu-section">主题</div>
          <div class="user-menu-item" data-set-theme="dark">
            <span class="ic">●</span><span class="lbl">Dark</span><span class="val"></span>
          </div>
          <div class="user-menu-item" data-set-theme="light">
            <span class="ic">○</span><span class="lbl">Light</span><span class="val"></span>
          </div>
          <div class="user-menu-item" data-set-theme="matrix">
            <span class="ic">⌬</span><span class="lbl">Matrix</span><span class="val"></span>
          </div>
          <div class="user-menu-divider"></div>
          <div class="user-menu-section">语言</div>
          <div class="user-menu-item" data-set-lang="zh">
            <span class="ic">文</span><span class="lbl">中文</span><span class="val"></span>
          </div>
          <div class="user-menu-item" data-set-lang="en">
            <span class="ic">EN</span><span class="lbl">English</span><span class="val"></span>
          </div>
          <div class="user-menu-divider"></div>
          <a class="user-menu-item" data-nav="settings.html" href="settings.html">
            <span class="ic">⚙</span><span class="lbl">设置</span><span class="val">⌘,</span>
          </a>
          <div class="user-menu-item" data-open-palette>
            <span class="ic">⌘</span><span class="lbl">命令面板</span><span class="val">⌘K</span>
          </div>
          <div class="user-menu-item" data-open-notif>
            <span class="ic">♪</span><span class="lbl">通知中心</span><span class="val">3</span>
          </div>
          <a class="user-menu-item" href="help.html">
            <span class="ic">?</span><span class="lbl">帮助</span><span class="val"></span>
          </a>
          <div class="user-menu-divider"></div>
          <div class="user-menu-item" data-disconnect>
            <span class="ic">⏻</span><span class="lbl" style="color:var(--error)">断开连接</span><span class="val"></span>
          </div>
          <div class="user-menu-foot">
            <span class="pulse"></span>
            <span>Polymarket · mainnet</span>
          </div>
        </div>
        <button class="user-card" data-user-card>
          <span class="avatar">${initials}</span>
          <span class="info">
            <span class="name">${ens}</span>
            <span class="addr">${short}</span>
          </span>
          <span class="chev">⌃</span>
        </button>
      `;
    } else {
      sidebarFoot.innerHTML = `
        <button class="user-connect" data-connect>
          <span class="ic">⏻</span>
          <span class="label">连接 Polymarket</span>
        </button>
      `;
    }
  }

  const connectBtn = document.querySelector('[data-connect]');
  if (connectBtn) {
    connectBtn.addEventListener('click', (e) => {
      e.preventDefault();
      const mock = '0x7a3f9b1c4d5e6f8a9b0c1d2e3f4a5b6c7d8e9f8b2c';
      localStorage.setItem('polyrocket.wallet', mock);
      location.reload();
    });
  }
  const disconnectBtn = document.querySelector('[data-disconnect]');
  if (disconnectBtn) {
    disconnectBtn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      localStorage.removeItem('polyrocket.wallet');
      location.reload();
    });
  }

  // command palette (⌘K) — minimal mock
  const palette = document.querySelector('[data-palette]');
  function openPalette() {
    if (!palette) return;
    palette.classList.add('open');
    const input = palette.querySelector('input');
    if (input) { input.value = ''; setTimeout(() => input.focus(), 30); }
  }
  function closePalette() { if (palette) palette.classList.remove('open'); }
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault(); openPalette();
    }
    if (e.key === 'Escape') closePalette();
  });
  document.querySelectorAll('[data-open-palette]').forEach((b) => {
    b.addEventListener('click', (e) => { e.preventDefault(); openPalette(); });
  });
  if (palette) {
    palette.addEventListener('click', (e) => {
      if (e.target === palette) { closePalette(); return; }
      const row = e.target.closest('.row[data-href]');
      if (row) {
        const href = row.getAttribute('data-href');
        closePalette();
        location.href = href;
      }
    });
  }

  // notifications toggle (mock — opens a tiny strip)
  const notif = document.querySelector('[data-notif]');
  document.querySelectorAll('[data-open-notif]').forEach((b) => {
    b.addEventListener('click', (e) => {
      e.preventDefault();
      if (notif) notif.classList.toggle('open');
    });
  });
  if (notif) {
    notif.addEventListener('click', (e) => {
      if (e.target === notif) notif.classList.remove('open');
    });
  }

  // number animations
  document.querySelectorAll('[data-num]').forEach((el) => {
    const target = parseFloat(el.getAttribute('data-num'));
    const fmt = el.getAttribute('data-fmt') || '0';
    const dur = 600;
    const start = performance.now();
    const from = 0;
    function tick(now) {
      const t = Math.min(1, (now - start) / dur);
      const v = from + (target - from) * (1 - Math.pow(1 - t, 3));
      el.textContent = fmtFloat(v, fmt);
      if (t < 1) requestAnimationFrame(tick);
      else el.textContent = fmtFloat(target, fmt);
    }
    requestAnimationFrame(tick);
  });
  function fmtFloat(v, spec) {
    if (spec.endsWith('%')) return v.toFixed(parseInt(spec) || 1) + '%';
    if (spec === 'usd') return '$' + v.toLocaleString(undefined, { maximumFractionDigits: 0 });
    if (spec === 'usd2') return '$' + v.toLocaleString(undefined, { maximumFractionDigits: 2 });
    const d = parseInt(spec) || 0;
    return v.toFixed(d);
  }

  // fusion bar markers
  document.querySelectorAll('[data-fusion]').forEach((el) => {
    const d = JSON.parse(el.getAttribute('data-fusion'));
    const m1 = document.createElement('div'); m1.className = 'marker';
    m1.style.left = d.market + '%';
    m1.title = 'Market ' + d.market + '%';
    el.appendChild(m1);
    const m2 = document.createElement('div'); m2.className = 'marker brand';
    m2.style.left = d.fused + '%';
    m2.title = 'Fused ' + d.fused + '%';
    el.appendChild(m2);
    const ticks = document.createElement('div'); ticks.className = 'ticks';
    for (let i = 1; i < 10; i++) {
      const s = document.createElement('span');
      s.style.left = (i * 10) + '%';
      ticks.appendChild(s);
    }
    el.appendChild(ticks);
  });

  const cssVar = (name) => getComputedStyle(root).getPropertyValue(name).trim();

  // sparkline
  document.querySelectorAll('canvas[data-spark]').forEach((cv) => {
    const data = JSON.parse(cv.getAttribute('data-spark'));
    const ctx = cv.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    const min = Math.min(...data), max = Math.max(...data);
    const pad = 2;
    const stroke = cssVar('--ink') || '#26251e';
    const fill = stroke + (savedTheme === 'light' ? '14' : '20');
    ctx.lineWidth = 1.25;
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, fill);
    grad.addColorStop(1, fill.replace(/[0-9a-f]{2}$/i, '00'));
    ctx.beginPath();
    data.forEach((v, i) => {
      const x = (i / (data.length - 1)) * (w - pad * 2) + pad;
      const y = h - pad - ((v - min) / (max - min || 1)) * (h - pad * 2);
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.strokeStyle = stroke; ctx.stroke();
    ctx.lineTo(w - pad, h); ctx.lineTo(pad, h); ctx.closePath();
    ctx.fillStyle = grad; ctx.fill();
  });

  // candlestick
  document.querySelectorAll('canvas[data-candle]').forEach((cv) => {
    const data = JSON.parse(cv.getAttribute('data-candle'));
    const ctx = cv.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    const min = Math.min(...data.map(c => c.l));
    const max = Math.max(...data.map(c => c.h));
    const pad = 6;
    const cw = (w - pad * 2) / data.length;
    const up = cssVar('--success') || '#1f8a65';
    const dn = cssVar('--error')   || '#cf2d56';
    data.forEach((c, i) => {
      const x = pad + i * cw + cw * 0.15;
      const yh = h - pad - ((c.h - min) / (max - min || 1)) * (h - pad * 2);
      const yl = h - pad - ((c.l - min) / (max - min || 1)) * (h - pad * 2);
      const yo = h - pad - ((c.o - min) / (max - min || 1)) * (h - pad * 2);
      const yc = h - pad - ((c.c - min) / (max - min || 1)) * (h - pad * 2);
      const isUp = c.c >= c.o;
      ctx.strokeStyle = isUp ? up : dn;
      ctx.fillStyle   = isUp ? up : dn;
      ctx.beginPath();
      ctx.moveTo(x + cw * 0.35, yh);
      ctx.lineTo(x + cw * 0.35, yl);
      ctx.stroke();
      const top = Math.min(yo, yc), bot = Math.max(yo, yc);
      ctx.fillRect(x, top, cw * 0.7, Math.max(1, bot - top));
    });
  });

  // donut
  document.querySelectorAll('canvas[data-donut]').forEach((cv) => {
    const data = JSON.parse(cv.getAttribute('data-donut'));
    const ctx = cv.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    const cx = w / 2, cy = h / 2, r = Math.min(w, h) / 2 - 4, ir = r * 0.6;
    const total = data.reduce((s, d) => s + d.v, 0) || 1;
    const palette = [
      cssVar('--tl-read'),
      cssVar('--tl-grep'),
      cssVar('--tl-done'),
      cssVar('--tl-edit'),
      cssVar('--tl-thinking'),
      cssVar('--muted'),
    ];
    let a = -Math.PI / 2;
    data.forEach((d, i) => {
      const seg = (d.v / total) * Math.PI * 2;
      ctx.beginPath();
      ctx.moveTo(cx, cy);
      ctx.arc(cx, cy, r, a, a + seg);
      ctx.closePath();
      ctx.fillStyle = d.c || palette[i % palette.length];
      ctx.fill();
      a += seg;
    });
    ctx.globalCompositeOperation = 'destination-out';
    ctx.beginPath(); ctx.arc(cx, cy, ir, 0, Math.PI * 2); ctx.fill();
    ctx.globalCompositeOperation = 'source-over';
  });

  // gauge
  document.querySelectorAll('canvas[data-gauge]').forEach((cv) => {
    const v = parseFloat(cv.getAttribute('data-gauge'));
    const ctx = cv.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    const cx = w / 2, cy = h * 0.75, r = Math.min(w / 2, h) - 6;
    const start = Math.PI, end = Math.PI * 2;
    const err = cssVar('--error') || '#cf2d56';
    const done = cssVar('--tl-done') || '#c08532';
    const cs = cssVar('--canvas-soft') || '#fafaf7';
    const ink = cssVar('--ink') || '#26251e';
    const colors = [err, done, cs, done, cssVar('--success') || '#1f8a65'];
    const stops = 5;
    for (let i = 0; i < stops; i++) {
      const a1 = start + ((end - start) * i) / stops;
      const a2 = start + ((end - start) * (i + 1)) / stops;
      ctx.beginPath();
      ctx.arc(cx, cy, r, a1, a2);
      ctx.lineWidth = 8; ctx.strokeStyle = colors[i]; ctx.stroke();
    }
    const ang = start + ((v + 3) / 6) * (end - start);
    ctx.beginPath();
    ctx.arc(cx, cy, r, ang - 0.02, ang + 0.02);
    ctx.lineWidth = 2; ctx.strokeStyle = ink; ctx.stroke();
  });
})();
