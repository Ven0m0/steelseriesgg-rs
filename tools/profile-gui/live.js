// Live lighting panel: the only part of this page that talks to a device, and only when the page was served by
// `ssgg gui`. Served from a file or any other server there is no token, so the panel never appears. Plain script
// (not a module) for the same reason app.js is: a page opened from the filesystem refuses module scripts.
'use strict';

(function () {
  const HEX = /^#?([0-9a-fA-F]{6})$/;

  // Six hex digits, upper case, no '#': the form `ssgg rgb color` takes. Null when the text is not a colour.
  function normalizeHex(text) {
    const match = HEX.exec(String(text).trim());
    return match ? match[1].toUpperCase() : null;
  }

  // `ssgg gui` serves the page with its per-run token in a meta tag; no tag means no live lighting.
  function tokenFrom(doc) {
    const meta = doc.querySelector('meta[name="ssgg-live-token"]');
    return meta ? meta.getAttribute('content') : null;
  }

  async function call(token, path, body) {
    const post = body !== undefined;
    const headers = { 'X-Ssgg-Token': token };
    if (post) headers['Content-Type'] = 'application/json';
    const res = await fetch(path, { method: post ? 'POST' : 'GET', headers, body: post ? JSON.stringify(body) : undefined });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
    return data;
  }

  function node(tag, props, children) {
    const el = document.createElement(tag);
    Object.assign(el, props);
    for (const child of children || []) el.append(child);
    return el;
  }

  // Fills `root` and shows it when this page can drive a keyboard; leaves it hidden otherwise.
  async function mount(root, doc) {
    const token = tokenFrom(doc);
    if (!token) return false;
    let status;
    try {
      status = await call(token, '/api/status');
    } catch {
      return false;
    }
    if (!status.keyboard) return false;

    const picker = node('input', { type: 'color', value: '#ff0000', ariaLabel: 'Colour' });
    const hex = node('input', { type: 'text', value: 'FF0000', size: 8, maxLength: 7, ariaLabel: 'Colour as hex' });
    const level = node('input', { type: 'range', min: 0, max: 100, value: 100, ariaLabel: 'Brightness' });
    const levelText = node('span', { className: 'mono', textContent: '100%' });
    const result = node('p', { className: 'mono note' });
    result.setAttribute('aria-live', 'polite');
    const buttons = [];

    async function send(path, body, done) {
      for (const b of buttons) b.disabled = true;
      try {
        await call(token, path, body);
        result.textContent = `${done} (the keyboard accepted the call; check the keys - lighting is untested on most boards)`;
      } catch (e) {
        result.textContent = `failed: ${e.message}`;
      } finally {
        for (const b of buttons) b.disabled = false;
      }
    }

    function sendColor() {
      const value = normalizeHex(hex.value);
      if (!value) {
        result.textContent = 'colour must be six hex digits, for example FF8800';
        return;
      }
      return send('/api/rgb/color', { color: value }, `colour ${value} sent`);
    }

    function button(label, onClick) {
      const b = node('button', { type: 'button', className: 'btn', textContent: label });
      b.addEventListener('click', onClick);
      buttons.push(b);
      return b;
    }

    picker.addEventListener('input', () => { hex.value = picker.value.slice(1).toUpperCase(); });
    picker.addEventListener('change', sendColor);
    hex.addEventListener('input', () => {
      const value = normalizeHex(hex.value);
      if (value) picker.value = `#${value.toLowerCase()}`;
    });
    hex.addEventListener('keydown', (event) => { if (event.key === 'Enter') sendColor(); });
    level.addEventListener('input', () => { levelText.textContent = `${level.value}%`; });
    level.addEventListener('change', () => send('/api/rgb/brightness', { level: Number(level.value) }, `brightness ${level.value}% sent`));

    root.replaceChildren(
      node('h2', {}, ['Live lighting ', node('span', { className: 'tag', textContent: 'experimental' })]),
      node('p', { className: 'mono note', textContent: `Drives ${status.keyboard} through ssgg. Close GG first; it overwrites other programs.` }),
      node('div', { className: 'live-row' }, [node('label', { textContent: 'Colour ' }), picker, hex, button('Set colour', sendColor), button('Off', () => send('/api/rgb/off', {}, 'off sent'))]),
      node('div', { className: 'live-row' }, [node('label', { textContent: 'Brightness ' }), level, levelText]),
      result,
    );
    root.hidden = false;
    return true;
  }

  globalThis.SsggLive = { call, mount, normalizeHex, tokenFrom };

  if (typeof document !== 'undefined') {
    const root = document.getElementById('liveBox');
    if (root) mount(root, document);
  }
})();
