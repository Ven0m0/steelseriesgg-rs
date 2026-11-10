// Device control panel: every `ssgg` subcommand, as a form. Each entry is the argv of the CLI command plus its
// fields, and Run sends that argv to `/api/cli`, which runs the real binary - so the page can never do more than
// the CLI does. Commands that run until stopped have their own group and are killed after 60 s. Like live.js it
// appears only when `ssgg-gui` served the page.
// Plain script (not a module) for the same reason app.js is.
'use strict';

(function () {
  // Field helpers. `flag` set: the value follows that flag (a checkbox just adds it); unset: a positional.
  const num = (name, value, extra) => ({ name, type: 'number', value, ...extra });
  const text = (name, value, extra) => ({ name, type: 'text', value: value || '', ...extra });
  const pick = (name, options, extra) => ({ name, type: 'select', options, value: options[0], ...extra });
  const check = (name, flag, extra) => ({ name, type: 'check', flag, ...extra });
  const dry = () => check('dry run (send nothing)', '--dry-run');
  const second = () => check('second actuation layer', '--second');
  const pid = () => text('product ID (hex, optional)', '', { flag: '--device', optional: true });
  const cmd = (label, args, fields, extra) => ({ label, args, fields: fields || [], ...extra });

  const CHANNELS = ['master', 'game', 'chat', 'media', 'aux', 'mic'];
  const KEY_HINT = 'KEY=MM pairs separated by spaces, e.g. W=1.0 A=2.0';

  const GROUPS = [
    ['Actuation (Apex Pro TKL 2023, experimental)', [
      cmd('Set all keys (mm)', ['actuation', 'set'], [num('mm', 1.0, { step: 0.1, min: 0.1, max: 4 }), second(), dry()]),
      cmd('Set all keys (0.1 mm units)', ['actuation', 'set-value'], [num('value', 10, { min: 1, max: 40 }), second(), dry()]),
      cmd('Set single keys', ['actuation', 'set-key'], [text('keys', 'W=1.0', { list: true, hint: KEY_HINT }), second(), dry()]),
      cmd('Clear second layer', ['actuation', 'clear-second'], [dry()]),
      cmd('Release mode', ['actuation', 'release-mode'], [pick('mode', ['0', '2', '3', '4']), dry()]),
      cmd('Rapid trigger sensitivity', ['actuation', 'rapid-trigger'], [num('sensitivity', 2, { min: 0, max: 255 }), dry()]),
      cmd('Rapid tap on/off', ['actuation', 'rapid-tap'], [pick('state', ['on', 'off']), dry()]),
      cmd('Rapid tap pairs', ['actuation', 'rapid-tap-pairs'], [text('pairs', '', { list: true, optional: true, hint: 'hex KEY1:KEY2[:MODE[:both]] separated by spaces, empty clears' }), dry()]),
    ]],
    ['RGB', [
      cmd('Colour', ['rgb', 'color'], [text('colour (hex or name)', 'FF0000')]),
      cmd('Brightness', ['rgb', 'brightness'], [num('level 0-100', 100, { min: 0, max: 100 })]),
      cmd('Effect', ['rgb', 'effect'], [pick('name', ['static', 'breathing', 'spectrum', 'wave', 'off']), num('speed', 1.0, { flag: '--speed', step: 0.1, min: 0.1, max: 5 })]),
      cmd('Off', ['rgb', 'off']),
      cmd('Release direct mode (0x1628)', ['rgb', 'release']),
      cmd('Per-key: one key', ['rgb', 'perkey', 'set-key'], [text('key', 'A'), text('colour', 'red')]),
      cmd('Per-key: several keys', ['rgb', 'perkey', 'set-keys'], [text('key:colour,key:colour', 'A:red,S:green')]),
      cmd('Per-key: HID range', ['rgb', 'perkey', 'set-region'], [num('start HID', 4), num('count', 4), text('colour', 'blue')]),
      cmd('Per-key: clear', ['rgb', 'perkey', 'clear']),
      cmd('Per-key: test one HID code', ['rgb', 'perkey', 'test-matrix'], [num('HID code', 4), text('colour', 'red')]),
      cmd('Per-key: test pattern', ['rgb', 'perkey', 'test-pattern'], [pick('pattern', ['rainbow', 'checkerboard', 'wave', 'test'])]),
      cmd('Per-key: show mapping', ['rgb', 'perkey', 'show-mapping']),
      cmd('Per-key: status', ['rgb', 'perkey', 'status']),
    ]],
    ['Devices', [
      cmd('List devices', ['devices']),
      cmd('Status', ['status'], [pick('device filter', ['all', 'keyboard', 'headset'], { flag: '--device' })]),
      cmd('Validate', ['validate'], [check('benchmark', '--benchmark'), num('timeout (s)', 30, { flag: '--timeout' }), text('export file', '', { flag: '--output', optional: true }), check('JSON', '--json')]),
      cmd('Test device', ['test-device'], [text('device name or path', ''), check('benchmark', '--benchmark'), check('verbose', '--verbose')]),
      cmd('Bug report', ['bug-report'], [text('output file', 'ssgg_bug_report.json', { flag: '--output' }), check('include HID logs', '--include-hid-logs')]),
    ]],
    ['Headset', [
      cmd('Status', ['headset', 'status']),
      cmd('Sidetone', ['headset', 'sidetone'], [num('level 0-100', 50, { min: 0, max: 100 })]),
      cmd('Auto-off', ['headset', 'auto-off'], [num('minutes (0 = off)', 10, { min: 0, max: 255 })]),
      cmd('Mic volume', ['headset', 'mic-volume'], [num('level 0-100', 50, { min: 0, max: 100 })]),
    ]],
    ['Mouse', [
      cmd('Status', ['mouse', 'status']),
      cmd('DPI', ['mouse', 'dpi'], [num('value', 800, { min: 100, max: 18000, step: 100 })]),
      cmd('Poll rate', ['mouse', 'poll-rate'], [pick('Hz', ['1000', '500', '250', '125'])]),
      cmd('Colour', ['mouse', 'color'], [text('colour', 'red')]),
    ]],
    ['OLED (Apex Pro TKL 2023, experimental)', [
      cmd('Show frame file', ['oled', 'show'], [text('640-byte file', ''), dry()]),
      cmd('Reset', ['oled', 'reset'], [dry()]),
    ]],
    ['Profiles', [
      cmd('List', ['profile', 'list']),
      cmd('Load', ['profile', 'load'], [text('name', '')]),
      cmd('Save', ['profile', 'save'], [text('name', '')]),
      cmd('Delete', ['profile', 'delete'], [text('name', '')], { danger: true }),
    ]],
    ['On-board profile', [
      cmd('Read', ['onboard', 'read'], [num('config number', 1, { flag: '--profile' }), pid(), text('JSON out file', '', { flag: '--out', optional: true }), text('image out file', '', { flag: '--bin', optional: true }), check('hex dump', '--hex'), check('read twice and compare', '--verify')]),
      cmd('Decode image file', ['onboard', 'decode'], [text('image file', ''), text('JSON out file', '', { flag: '--out', optional: true })]),
      cmd('Encode JSON file', ['onboard', 'encode'], [text('JSON file', ''), text('out file', '', { flag: '--out', optional: true }), text('compare with image', '', { flag: '--check', optional: true })]),
      cmd('Write to keyboard', ['onboard', 'write'], [text('JSON file', ''), num('config number', 1, { flag: '--profile' }), pid(), dry(), check('confirm the write (--yes)', '--yes')], { danger: true }),
      cmd('Run a config slot', ['onboard', 'load'], [num('config number', 1, { flag: '--profile' }), pid()], { danger: true }),
      cmd('Gen3 read (0x1628, experimental)', ['onboard', 'gen3-read'], [num('slot 0-4', 0, { flag: '--slot' }), text('blob out file', '', { flag: '--bin', optional: true }), dry()]),
    ]],
    ['Polling rate (needs elevated rights)', [
      cmd('Status', ['pollrate', 'status']),
      cmd('Mouse', ['pollrate', 'mouse'], [pick('Hz', ['1000', '500', '250', '125', '2000', '4000']), check('persistent', '--persistent')], { danger: true }),
      cmd('Keyboard', ['pollrate', 'keyboard'], [pick('Hz', ['1000', '500', '250', '125', '2000', '4000']), check('persistent', '--persistent')], { danger: true }),
    ]],
    ['Audio (build feature `audio`)', [
      cmd('Status', ['audio', 'status']),
      cmd('Volume', ['audio', 'volume'], [pick('channel', CHANNELS), num('level 0-100', 50, { min: 0, max: 100 })]),
      cmd('Mute / unmute / toggle', ['audio', 'mute'], [pick('channel', CHANNELS), pick('state', ['toggle', 'true', 'false'], { flag: '--state', skip: 'toggle' })]),
      cmd('Chat mix', ['audio', 'chat-mix'], [num('balance -100..100', 0, { min: -100, max: 100 })]),
    ]],
    ['Sonar (build feature `sonar`)', [
      cmd('Status', ['sonar', 'status']),
      cmd('Discover', ['sonar', 'discover']),
      cmd('Devices', ['sonar', 'devices']),
      cmd('Mode', ['sonar', 'mode']),
      cmd('Volume', ['sonar', 'volume'], [pick('channel', CHANNELS.slice(0, 5)), num('level 0-100', 50, { min: 0, max: 100 })]),
      cmd('Chat mix', ['sonar', 'chat-mix']),
      cmd('Streamer: monitoring volume', ['sonar', 'streamer', 'monitoring'], [pick('channel', ['master', 'game', 'chat']), num('level 0-100', 50, { min: 0, max: 100 })]),
      cmd('Streamer: streaming volume', ['sonar', 'streamer', 'streaming'], [pick('channel', ['master', 'game', 'chat']), num('level 0-100', 50, { min: 0, max: 100 })]),
      cmd('Configs', ['sonar', 'configs']),
    ]],
    ['Performance', [
      cmd('Stats', ['performance', 'stats'], [check('JSON', '--json')]),
      cmd('Enable', ['performance', 'enable']),
      cmd('Disable', ['performance', 'disable']),
      cmd('Cleanup', ['performance', 'cleanup']),
      cmd('Benchmark', ['performance', 'benchmark'], [num('duration (s)', 10, { flag: '--duration' })]),
      cmd('Verify RGB performance', ['verify-performance'], [num('duration (s)', 30, { flag: '--duration' }), text('effect', 'breathing', { flag: '--effect' }), text('export file', '', { flag: '--output', optional: true })]),
    ]],
    ['Key remapper', [
      cmd('List bindings', ['remap', 'list']),
    ]],
    ['Long-running commands (stopped after 60 s)', [
      cmd('Remap keys (grabs the keyboard)', ['remap', 'run'], [], { note: 'Runs until Ctrl-C or the escape chord. The panel stops it after 60 s, so use the CLI for a long session.', danger: true }),
      cmd('GameSense server', ['server'], [num('port', 27301, { flag: '--port' })], { note: 'Serves GameSense on that port until stopped.' }),
      cmd('Daemon', ['daemon'], [], { note: 'Runs the device daemon until stopped.' }),
    ]],
  ];

  // argv for one command from `values` (field index -> string or boolean). Throws a message for a missing
  // required value. Positionals go after `--` only when one starts with '-' (a negative chat-mix balance).
  function buildArgs(command, values) {
    const head = [...command.args];
    const positional = [];
    command.fields.forEach((field, i) => {
      const raw = values[i];
      if (field.type === 'check') {
        if (raw) head.push(field.flag);
        return;
      }
      const value = String(raw === undefined ? '' : raw).trim();
      if (value === '' || value === field.skip) {
        if (field.optional || field.skip) return;
        throw new Error(`${field.name} is required`);
      }
      if (field.flag) head.push(field.flag, value);
      else if (field.list) positional.push(...value.split(/\s+/));
      else positional.push(value);
    });
    return positional.some((p) => p.startsWith('-')) ? [...head, '--', ...positional] : [...head, ...positional];
  }

  function node(tag, props, children) {
    const el = document.createElement(tag);
    Object.assign(el, props);
    for (const child of children || []) el.append(child);
    return el;
  }

  async function mount(root, doc) {
    const live = globalThis.SsggLive;
    const token = live && live.tokenFrom(doc);
    if (!token) return false;

    const group = node('select', { ariaLabel: 'Command group' });
    GROUPS.forEach(([title], i) => group.append(node('option', { value: String(i), textContent: title })));
    const which = node('select', { ariaLabel: 'Command' });
    const form = node('div', { className: 'ctl-form' });
    const run = node('button', { type: 'button', className: 'btn primary', textContent: 'Run' });
    const output = node('pre', { className: 'mono ctl-out' });
    output.setAttribute('aria-live', 'polite');
    let armed = false;
    let inputs = [];

    const current = () => GROUPS[Number(group.value)][1][Number(which.value)];
    const disarm = () => { armed = false; run.textContent = 'Run'; run.classList.remove('warn'); };

    function fillCommands() {
      which.replaceChildren(...GROUPS[Number(group.value)][1].map((c, i) => node('option', { value: String(i), textContent: c.label })));
      fillForm();
    }

    function fillForm() {
      disarm();
      inputs = current().fields.map((field) => {
        let input;
        if (field.type === 'check') input = node('input', { type: 'checkbox' });
        else if (field.type === 'select') input = node('select', {}, field.options.map((o) => node('option', { value: o, textContent: o })));
        else input = node('input', { type: field.type, value: field.value === undefined ? '' : String(field.value) });
        if (field.type === 'number') for (const k of ['min', 'max', 'step']) if (field[k] !== undefined) input[k] = field[k];
        input.ariaLabel = field.name;
        input.addEventListener('input', disarm);
        return input;
      });
      const rows = current().fields.map((field, i) => node('div', { className: 'live-row' }, [
        node('label', { textContent: `${field.name} ` }), inputs[i], ...(field.hint ? [node('span', { className: 'mono note', textContent: field.hint })] : []),
      ]));
      form.replaceChildren(...rows);
      if (current().danger) form.append(node('p', { className: 'mono note', textContent: 'This changes the device or system settings: the first click arms the button, the second runs it.' }));
      if (current().note) form.append(node('p', { className: 'mono note', textContent: current().note }));
    }

    run.addEventListener('click', async () => {
      const command = current();
      let args;
      try {
        args = buildArgs(command, inputs.map((input, i) => (command.fields[i].type === 'check' ? input.checked : input.value)));
      } catch (e) {
        output.textContent = e.message;
        return;
      }
      if (command.danger && !armed) {
        armed = true;
        run.textContent = 'Click again to run';
        run.classList.add('warn');
        return;
      }
      disarm();
      run.disabled = true;
      output.textContent = `$ ssgg ${args.join(' ')}\n...`;
      try {
        const r = await live.call(token, '/api/cli', { args });
        const tail = r.timed_out ? '\n[stopped after 60 s]' : `\n[exit ${r.code}]`;
        output.textContent = `$ ssgg ${args.join(' ')}\n${r.stdout}${r.stderr}${tail}`;
      } catch (e) {
        output.textContent = `failed: ${e.message}`;
      } finally {
        run.disabled = false;
      }
    });
    group.addEventListener('change', fillCommands);
    which.addEventListener('change', fillForm);

    root.replaceChildren(
      node('h2', {}, ['Device control ', node('span', { className: 'tag', textContent: 'runs the ssgg CLI' })]),
      node('p', { className: 'mono note', textContent: 'Every action of the command line, run through it. Close GG first; it overwrites other programs. Most device protocols are experimental and untested on hardware.' }),
      node('div', { className: 'live-row' }, [group, which, run]),
      form,
      output,
    );
    fillCommands();
    root.hidden = false;
    return true;
  }

  globalThis.SsggControl = { GROUPS, buildArgs, mount };

  if (typeof document !== 'undefined') {
    const root = document.getElementById('controlBox');
    if (root) mount(root, document);
  }
})();
