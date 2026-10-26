// Headset tab: battery, connection and ChatMix, plus sidetone, mic volume and auto-off. Like live.js it appears only
// when `ssgg-gui` served the page and opened a headset. Plain script (not a module) for the same reason app.js is.
// The headset cannot report its sidetone, mic volume or auto-off, so the controls start from the values saved in
// config.toml (the daemon applies those whenever the headset powers on) and otherwise show "not set".
'use strict';

(function () {
  // Fast enough for the ChatMix dial to follow the hand; one status query is a few milliseconds of HID traffic.
  const POLL_MS = 500;
  const LOW_BATTERY = 15;
  const SIDETONE_NAMES = ['Off', 'Low', 'Med', 'High'];

  // The 0-100 level for step `step` of `steps`, rounded to nearest: the inverse of `scale_to_wire` in nova7.rs,
  // so every step reaches its own wire value (the Rust test `every_capability_step_reaches_its_wire_value`).
  function levelForStep(step, steps) {
    return Math.round((step * 100) / (steps - 1));
  }

  // The step a 0-100 level lands on, as `scale_to_wire` maps it.
  function stepForLevel(level, steps) {
    return Math.floor((level * (steps - 1) + 50) / 100);
  }

  function stepLabels(steps) {
    return steps === SIDETONE_NAMES.length ? SIDETONE_NAMES.slice() : Array.from({ length: steps }, (_, i) => String(i));
  }

  // ChatMix -1 (full game) .. +1 (full chat) as a left offset in percent.
  function chatMixPercent(mix) {
    return Math.max(0, Math.min(100, (mix + 1) * 50));
  }

  function chatMixText(mix) {
    return `${mix > 0 ? '+' : ''}${mix.toFixed(2)}`;
  }

  function node(tag, props, children) {
    const el = document.createElement(tag);
    Object.assign(el, props);
    for (const child of children || []) el.append(child);
    return el;
  }

  // Show one view (keyboardView / headsetView) and mark its tab.
  function showView(doc, tabs, id) {
    for (const button of tabs.querySelectorAll('button[data-view]')) {
      const on = button.dataset.view === id;
      button.classList.toggle('active', on);
      button.setAttribute('aria-pressed', String(on));
      const view = doc.getElementById(button.dataset.view);
      if (view) view.hidden = !on;
    }
  }

  async function mount(doc) {
    const live = globalThis.SsggLive;
    const root = doc.getElementById('headsetView');
    const tabs = doc.getElementById('viewTabs');
    const token = live && live.tokenFrom(doc);
    if (!root || !tabs || !token) return false;
    let info;
    try {
      const status = await live.call(token, '/api/status');
      if (!status.headset) return false;
      info = await live.call(token, '/api/headset');
    } catch {
      return false;
    }
    const call = (path, body) => live.call(token, path, body);
    const caps = info.capabilities;
    const saved = info.saved || {};
    const pid = `1038:${info.product_id.toString(16).toUpperCase().padStart(4, '0')}`;
    const controls = [];
    const result = node('p', { className: 'mono note' });
    result.setAttribute('aria-live', 'polite');

    // --- status row ---
    const connText = node('strong', { textContent: '...' });
    const connDot = node('span', { className: 'hs-dot' });
    const battText = node('strong', { textContent: '...' });
    const battNote = node('span', { className: 'hint' });
    const battFill = node('div', { className: 'hs-fill' });
    const mixText = node('span', { className: 'mono', textContent: '' });
    const mixKnob = node('div', { className: 'hs-knob' });
    const polled = node('p', { className: 'mono note' });
    const offNote = node('p', { className: 'hs-off', hidden: true, textContent: 'Turn the headset on to change settings. Ticked settings are re-applied automatically when it connects.' });

    const cards = [
      node('div', { className: 'hs-card' }, [node('span', { className: 'hint', textContent: 'Connection' }), node('div', { className: 'hs-line' }, [connDot, connText])]),
      node('div', { className: 'hs-card' }, [
        node('span', { className: 'hint', textContent: 'Battery' }),
        node('div', { className: 'hs-line' }, [battText, battNote]),
        node('div', { className: 'hs-bar' }, [battFill]),
      ]),
    ];
    if (caps.chat_mix) {
      cards.push(node('div', { className: 'hs-card' }, [
        node('div', { className: 'hs-line spread' }, [node('span', { className: 'hint', textContent: 'ChatMix dial' }), mixText]),
        node('div', { className: 'hs-mix' }, [node('div', { className: 'hs-track' }), mixKnob]),
        node('div', { className: 'hs-line spread hint' }, [node('span', { textContent: 'Game' }), node('span', { textContent: 'Chat' })]),
      ]));
    }

    function setConnected(on) {
      connText.textContent = on === false ? 'Headset off' : on ? 'Connected' : 'Unknown';
      connDot.classList.toggle('on', on === true);
      offNote.hidden = on !== false;
      for (const c of controls) c.disabled = on === false;
    }

    function render(status) {
      setConnected(status.connected);
      const pct = status.battery_percent;
      battText.textContent = pct == null ? 'unknown' : `${pct}%`;
      battText.classList.toggle('low', pct != null && pct < LOW_BATTERY && !status.charging);
      battNote.textContent = status.charging ? 'charging' : status.charging === false ? 'not charging' : '';
      battFill.style.width = `${pct || 0}%`;
      battFill.classList.toggle('low', pct != null && pct < LOW_BATTERY && !status.charging);
      if (status.chat_mix != null) {
        mixText.textContent = chatMixText(status.chat_mix);
        mixKnob.style.left = `${chatMixPercent(status.chat_mix)}%`;
      }
    }

    // A slow answer can outlast POLL_MS; skip ticks while one query is out instead of queueing them on the server.
    let polling = false;
    async function poll() {
      if (polling) return;
      polling = true;
      try {
        render(await call('/api/headset/status'));
        polled.textContent = `status polled every ${POLL_MS / 1000} s while this tab is open · last update ${new Date().toLocaleTimeString()}`;
      } catch (e) {
        polled.textContent = `status failed: ${e.message}`;
      } finally {
        polling = false;
      }
    }

    // --- one setting: send it, keep or forget it in config.toml, and roll back on failure ---
    function setting(path, label, initial, show) {
      const state = { value: initial, persist: initial != null };
      const persist = node('input', { type: 'checkbox', checked: state.persist, disabled: initial == null });
      controls.push(persist);
      async function send(value, keep, done) {
        try {
          await call(path, { value, persist: keep });
          state.value = value;
          state.persist = keep;
          persist.disabled = false;
          result.textContent = `${label} set to ${done}${keep ? ', saved for power-on' : ''}`;
        } catch (e) {
          result.textContent = `${label} not changed: ${e.message}`;
        }
        persist.checked = state.persist;
        show(state.value);
      }
      persist.addEventListener('change', () => {
        if (state.value == null) {
          persist.checked = false;
          result.textContent = `set ${label.toLowerCase()} first, then it can be saved`;
          return;
        }
        send(state.value, persist.checked, 'the same value');
      });
      const row = node('label', { className: 'hs-persist' }, [persist, ' Re-apply when headset powers on']);
      return { state, send, row };
    }

    const fieldsets = [];

    if (caps.sidetone_steps > 0) {
      const steps = caps.sidetone_steps;
      const names = stepLabels(steps);
      const buttons = names.map((name) => node('button', { type: 'button', className: 'btn hs-step', textContent: name }));
      const show = (level) => buttons.forEach((b, i) => {
        const on = level != null && stepForLevel(level, steps) === i;
        b.classList.toggle('active', on);
        b.setAttribute('aria-pressed', String(on));
      });
      const s = setting('/api/headset/sidetone', 'Sidetone', saved.sidetone ?? null, show);
      buttons.forEach((b, i) => {
        controls.push(b);
        b.addEventListener('click', () => s.send(levelForStep(i, steps), s.state.persist, names[i]));
      });
      show(s.state.value);
      fieldsets.push(node('fieldset', { className: 'hs-set' }, [
        node('legend', { textContent: 'Sidetone' }),
        node('div', { className: 'hs-steps', role: 'group', ariaLabel: 'Sidetone level' }, buttons),
        node('span', { className: 'hint', textContent: `Hear your own voice. ${steps} steps on this model.` }),
        s.row,
      ]));
    }

    if (caps.mic_volume_steps > 0) {
      const steps = caps.mic_volume_steps;
      const range = node('input', { type: 'range', min: 0, max: steps - 1, step: 1, ariaLabel: 'Microphone volume' });
      const out = node('output', { className: 'mono' });
      const show = (level) => {
        if (level == null) {
          out.textContent = 'not set';
          range.value = String(steps - 1);
          return;
        }
        range.value = String(stepForLevel(level, steps));
        out.textContent = `${range.value} / ${steps - 1}`;
      };
      const s = setting('/api/headset/mic-volume', 'Mic volume', saved.mic_volume ?? null, show);
      controls.push(range);
      range.addEventListener('input', () => { out.textContent = `${range.value} / ${steps - 1}`; });
      range.addEventListener('change', () => s.send(levelForStep(Number(range.value), steps), s.state.persist, `${range.value} / ${steps - 1}`));
      show(s.state.value);
      fieldsets.push(node('fieldset', { className: 'hs-set' }, [
        node('legend', { textContent: 'Mic volume' }),
        node('div', { className: 'hs-line' }, [range, out]),
        node('span', { className: 'hint', textContent: `${steps} steps (0 to ${steps - 1}). 0 is close to silent, not a mute.` }),
        s.row,
      ]));
    }

    if (caps.auto_off) {
      const minutes = node('input', { type: 'number', min: 0, max: 255, step: 1, ariaLabel: 'Auto-off minutes' });
      const show = (value) => { minutes.value = value == null ? '' : String(value); };
      const s = setting('/api/headset/auto-off', 'Auto-off', saved.auto_off ?? null, show);
      controls.push(minutes);
      minutes.addEventListener('change', () => {
        const value = Number(minutes.value);
        if (minutes.value === '' || !Number.isInteger(value) || value < 0 || value > 255) {
          result.textContent = 'auto-off takes whole minutes from 0 to 255';
          show(s.state.value);
          return;
        }
        s.send(value, s.state.persist, value === 0 ? 'never' : `${value} min`);
      });
      show(s.state.value);
      fieldsets.push(node('fieldset', { className: 'hs-set' }, [
        node('legend', { textContent: 'Auto-off' }),
        node('div', { className: 'hs-line' }, [minutes, node('span', { className: 'hint', textContent: 'minutes idle, 0 = never' })]),
        s.row,
      ]));
    }

    const supported = fieldsets.length > 0 || caps.status;
    root.replaceChildren(node('section', { className: 'panel hs-panel' }, [
      node('div', { className: 'hs-head' }, [
        node('h2', {}, [`${info.name} `, node('span', { className: 'tag', textContent: '[EXPERIMENTAL]' })]),
        node('p', { className: 'mono note', textContent: `${pid} · close GG first, it overwrites other programs` }),
      ]),
      ...(supported ? [] : [node('p', { textContent: 'ssgg detects this headset but knows no commands for it. Nothing can be read or set yet.' })]),
      ...(caps.status ? [node('div', { className: 'hs-cards' }, cards), polled] : []),
      offNote,
      ...(fieldsets.length ? [node('div', { className: 'hs-sets' }, fieldsets)] : []),
      result,
      ...(fieldsets.length ? [node('p', { className: 'note', textContent: 'Settings change at once. Nothing is saved on the headset itself; ticked settings go to config.toml.' })] : []),
    ]));

    // Poll only while the headset tab is open and the browser tab is visible.
    const active = () => !root.hidden && doc.visibilityState !== 'hidden';
    if (caps.status) {
      setInterval(() => { if (active()) poll(); }, POLL_MS);
      doc.addEventListener('visibilitychange', () => { if (active()) poll(); });
    }
    for (const button of tabs.querySelectorAll('button[data-view]')) {
      button.addEventListener('click', () => {
        showView(doc, tabs, button.dataset.view);
        if (caps.status && active()) poll();
      });
    }
    tabs.hidden = false;
    return true;
  }

  globalThis.SsggHeadset = { chatMixPercent, chatMixText, levelForStep, mount, showView, stepForLevel, stepLabels };

  if (typeof document !== 'undefined') mount(document);
})();
