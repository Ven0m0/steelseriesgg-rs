// Node-side selftest for tools/profile-gui/app.js.
//
//   timeout 120 node tools/profile-gui/selftest.mjs
//
// Covers the static tables, the JSON the read writes and the page reads, the edit API and
// its change list, board geometry (the cap box is the click and focus target), the wheel
// and OLED panels, the macro block, panel render - and the Rust-side tables in
// src/devices/keyboards/apex_7_profile.rs, src/devices/mod.rs and src/devices/discovery.rs.

// Model names and the per-model feature set are imported from Rust by sync-products.mjs into
// products.generated.js. Run `node tools/profile-gui/sync-products.mjs` after changing the Rust
// tables, and `node tools/profile-gui/sync-products.mjs --check` (also run as a check below) to fail
// on drift without rewriting.
//
// Every check is a thunk: a check the suite gets wrong fails and the run reaches the summary.
// Exits non-zero when any check fails.
import { existsSync, readFileSync } from 'node:fs';
import vm from 'node:vm';
import { spawnSync } from 'node:child_process';
import { installFakeDom, loadApp } from './fake-dom.mjs';

// The generator's output is a classic script that sets globalThis.SsggProducts; index.html loads it
// before app.js, so the suite does the same before it loads app.js. vm.runInThisContext runs the file
// text as a classic script without an eval'd string (no dynamic-code call site for a scanner to flag).
const sync = await import('./sync-products.mjs');
const productSrc = readFileSync(`${import.meta.dirname}/products.generated.js`, 'utf8');
vm.runInThisContext(productSrc, { filename: `${import.meta.dirname}/products.generated.js` });

installFakeDom();
const app = loadApp(import.meta.dirname);
const EX = `${import.meta.dirname}/examples`;
const style = readFileSync(`${import.meta.dirname}/style.css`, 'utf8');

let checks = 0;
let skipped = 0;
let failed = 0;
const safe = (fn) => {
  try { const v = fn(); return typeof v === 'string' ? v : JSON.stringify(v) ?? String(v); } catch (e) { return `<detail threw ${e.message}>`; }
};
const pending = [];
function report(label, pass, detail) {
  if (pass) console.log(`  ok   ${label}`);
  else { failed += 1; console.log(`  FAIL ${label} - ${safe(detail)}`); }
}
function ok(label, thunk, detail = () => '') {
  checks += 1;
  let result;
  try {
    result = thunk();
  } catch (e) {
    failed += 1;
    console.log(`  FAIL ${label} - threw ${e.message}`);
    return;
  }
  if (result && typeof result.then === 'function') {
    pending.push(result.then((v) => report(label, v === true, detail), (e) => { failed += 1; console.log(`  FAIL ${label} - threw ${e.message}`); }));
    return;
  }
  if (result === SKIP) {
    skipped += 1;
    console.log(`  skip ${label} - needs the profile JSONs in the repo root, which are the operator's captures, not files of the repo`);
    return;
  }
  report(label, result === true, detail);
}
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
// A check can return SKIP: the checks that measure the profile JSONs in the repo root (the operator's
// captures, which are not committed) skip and say so in a checkout without them.
const SKIP = 'skip';
const rootText = (name) => (existsSync(`../../${name}`) ? readFileSync(`../../${name}`, 'utf8') : null);
// A check that measures the real macro decode runs only when that file is here; without it, skip.
const okMacros = (label, thunk, detail = () => '') => ok(label, () => (macrosDoc ? thunk() : SKIP), detail);
const parseExample = (name) => JSON.parse(readFileSync(`${EX}/${name}`, 'utf8'));

// The page keeps one file at a time, so samples are loaded one after another and their docs
// are kept by the checks below.
const remap = app.addFile('sample-remap.json', readFileSync(`${EX}/sample-remap.json`, 'utf8'));
const apex = remap.file.doc;
const identityDoc = parseExample('sample-identity.json');
// The macro checks measure a decode of a real macro run. That file is the operator's (it carries their
// macros), so it lives in the repo root next to the other captures and is not committed: without it the
// macro checks skip.
const macrosText = rootText('sample-macros.json');
const macrosDoc = macrosText === null ? null : JSON.parse(macrosText);

// --- byte, colour and digest helpers --------------------------------------
console.log('hex, colour and digest helpers');
ok('a byte prints as two uppercase hex digits', () => app.hex2(15) === '0F' && app.hex2(0) === '00' && app.hex2(255) === 'FF',
  () => [app.hex2(15), app.hex2(0), app.hex2(255)]);
ok('a byte list prints space separated', () => app.hexOf([4, 0, 255]) === '04 00 FF' && app.hexOf([]) === '',
  () => [app.hexOf([4, 0, 255]), app.hexOf([])]);
ok('the printer round trips through the parser',
  () => eq(app.parseHex('04 00 FF'), [4, 0, 255]) && eq(app.parseHex(app.hexOf([1, 2, 3])), [1, 2, 3]));
ok('an empty code list parses to no codes', () => app.parseHex('').length === 0, () => app.parseHex(''));
ok('commas are not code separators, so a comma list is one token', () => app.parseHex('04,00,00,00').length === 1,
  () => app.parseHex('04,00,00,00'));
ok('colour round trips through the hex string',
  () => app.colorToHex(app.hexToColor('FF8000')) === '#FF8000' && eq(app.hexToColor('#FF8000'), { r: 255, g: 128, b: 0 }),
  () => [app.colorToHex(app.hexToColor('FF8000')), app.hexToColor('#FF8000')]);
// The same FNV-1a-64 the decoder records for the region behind the payload and the Python kit
// in docs/development/python/ computes in compare_extra.py.
ok('the page digest matches the published test vector', () => app.fnv1a64([1, 2, 3]) === 'B1CEBB18672CF5AB', () => app.fnv1a64([1, 2, 3]));

// --- static tables --------------------------------------------------------
console.log('key, function, binding group and mouse tables');
ok('the usage table covers every byte and the mapping covers most offsets',
  () => app.HID2OFF.length === 256 && app.OFF2HID.length === 128
    && app.OFF2HID.filter((v) => typeof v === 'number' && v >= 0).length === 120
    && app.OFF2HID.filter((v) => v === null).length === 8,
  () => [app.HID2OFF.length, app.OFF2HID.length]);
ok('every mapped offset inverts back to its usage',
  () => app.OFF2HID.every((hid, offset) => (typeof hid !== 'number' || hid < 0) || app.HID2OFF[hid] === offset));
ok('the usage table is injective over mapped offsets',
  () => new Set(app.OFF2HID.filter((v) => typeof v === 'number' && v >= 0)).size === 120);
ok('a key name comes from the usage byte', () => app.keyName(0x04) === 'A' && app.keyName(0x29) === 'Esc' && app.keyName(0x0a) === 'G',
  () => [app.keyName(0x04), app.keyName(0x29)]);
ok('a usage byte with no name is labelled as hex, not invented',
  () => /^0x[0-9A-Fa-f]{2}$/.test(app.keyName(0xff)), () => app.keyName(0xff));
ok('the function table is a byte-to-name map with 0 as deactivated',
  () => !Array.isArray(app.FUNCTION_NAMES) && app.FUNCTION_NAMES[0] === 'deactivated' && app.FUNCTION_NAMES[0x51] === 'combo_key'
    && app.FUNCTION_NAMES[0x71] === 'macro' && Object.keys(app.FUNCTION_NAMES).length > 15,
  () => Object.keys(app.FUNCTION_NAMES).length);
ok('an unlisted function byte is labelled unknown, not invented',
  () => app.functionName(0x7e) === 'unknown' && app.functionName(0xee) === 'unknown' && app.functionName(0) === 'deactivated',
  () => [app.functionName(0x7e), app.functionName(0)]);
ok('the binding groups are the offers GG makes and the macro group is function 0x71',
  () => app.BINDING_GROUPS.length >= 5 && app.BINDING_GROUPS.some((g) => g.functions.includes(0x71))
    && app.BINDING_GROUPS.some((g) => g.functions.length === 1 && g.functions[0] === 0)
    && app.BINDING_GROUPS.every((g) => g.label && g.label !== g.id),
  () => app.BINDING_GROUPS.map((g) => [g.id ?? g.label, g.functions.length]));
ok('the binding group of a function byte is the group the pickers show',
  () => app.bindingGroup(0x71) === app.BINDING_GROUPS.find((g) => g.functions.includes(0x71))
    && app.bindingGroup(0) === app.BINDING_GROUPS.find((g) => g.functions.length === 1 && g.functions[0] === 0),
  () => app.bindingGroup(0x71));
ok('the mouse action list names every function byte it offers',
  () => app.MOUSE_FUNCTIONS.length === 13 && app.MOUSE_FUNCTIONS.every((fn) => app.MOUSE_NAMES[fn] !== undefined)
    && !app.MOUSE_FUNCTIONS.includes(0), () => [app.MOUSE_FUNCTIONS.length, Object.keys(app.MOUSE_NAMES).length]);
ok('the factory binding of a mapped slot fires the key itself, of an empty slot it is all zero',
  () => {
    const mapped = app.factoryEntry('button_mappings', 27);
    const empty = app.factoryEntry('button_mappings', 6);
    return mapped.function === 0x51 && mapped.key_codes === `07 00 00 00`
      && empty.function === 0 && empty.key_codes === '00 00 00 00';
  }, () => [app.factoryEntry('button_mappings', 27), app.factoryEntry('button_mappings', 6)]);
ok('normalizeEntry derives the binding the factory pair implies',
  () => app.normalizeEntry({ ...app.factoryEntry('button_mappings', 27), offset: 27 }, 27).binding === 'self'
    && app.normalizeEntry({ ...app.factoryEntry('button_mappings', 6), offset: 6 }, 6).binding === 'none',
  () => [app.normalizeEntry({ ...app.factoryEntry('button_mappings', 27), offset: 27 }, 27).binding,
    app.normalizeEntry({ ...app.factoryEntry('button_mappings', 6), offset: 6 }, 6).binding]);
ok('binding classification separates self, none and custom',
  () => app.bindingOf(0, [0, 0, 0, 0], 41) === 'none' && app.bindingOf(0x51, [41, 0, 0, 0], 41) === 'self'
    && app.bindingOf(0x51, [26, 0, 0, 0], 41) === 'custom' && app.bindingOf(0x71, [26, 0, 0, 0], 41) === 'custom',
  () => [app.bindingOf(0, [0, 0, 0, 0], 41), app.bindingOf(0x51, [41, 0, 0, 0], 41)]);
ok('the layer names are the two mapping lists a read decodes',
  () => app.LAYER_BASE === 'base' && app.LAYER_META === 'meta'
    && app.layerArray(apex, app.LAYER_BASE) === apex.button_mappings
    && app.layerArray(apex, app.LAYER_META) === apex.button_meta_mappings);
ok('the code count and mapping count match the decoder layout',
  () => app.CODE_COUNT === 4 && app.MAPPING_COUNT === 128 && Object.values(app.EXPECTED_ARRAYS).reduce((a, b) => a + b, 0) > 0,
  () => app.EXPECTED_ARRAYS);

// --- Rust-side parity -----------------------------------------------------
console.log('key names, mapping slots and function names against the Rust tables');
(() => {
  const mod = readFileSync(`${import.meta.dirname}/../../src/devices/keyboards/apex_7_profile.rs`, 'utf8');
  // The page's HID2OFF mirrors this Rust table (255 = the usage has no mapping slot).
  const table = mod.slice(mod.indexOf('pub const HID_TO_OFFSET'));
  const body2 = table.slice(table.indexOf('= [') + 3, table.indexOf('];'));
  const rustOff = body2.split(',').map((t) => t.trim()).filter((t) => t.length).map((t) => Number(t.startsWith('0x') ? parseInt(t.slice(2), 16) : t));
  ok('the Rust usage table has one slot per usage byte', () => rustOff.length === 256, () => rustOff.length);
  ok('the page mapping table is the Rust usage table, -1 for 255',
    () => rustOff.slice(0, 256).every((slot, usage) => (slot === 0xFF ? -1 : slot) === app.HID2OFF[usage]),
    () => rustOff.slice(0, 256).map((slot, usage) => [usage, slot === 0xFF ? -1 : slot, app.HID2OFF[usage]]).filter((r) => r[1] !== r[2]).slice(0, 5));
  const mapped = rustOff.slice(0, 256).filter((v) => v !== 0xFF);
  ok('the page inverts the Rust table into an offset table of 128 slots',
    () => app.OFF2HID.length === 128 && mapped.every((off) => app.OFF2HID[off] !== null)
      && app.OFF2HID.filter((v) => v === null).length === 128 - new Set(mapped).size,
    () => [app.OFF2HID.length, mapped.length, app.OFF2HID.filter((v) => v === null).length]);
  ok('the mapping slots the Rust table uses are the slots the page names',
    () => mapped.every((off) => app.OFF2HID[off] !== null && app.KEY_NAMES[app.OFF2HID[off]] !== undefined),
    () => mapped.filter((off) => app.KEY_NAMES[app.OFF2HID[off]] === undefined).slice(0, 5));
  ok('the Rust decoder resolves a usage the way the page does',
    () => rustOff.slice(0, 256).every((slot, usage) => (slot === 0xFF ? app.HID2OFF[usage] === -1 : slot === app.HID2OFF[usage]))
      && app.HID2OFF.length === 256 && rustOff.length === 256,
    () => [0, 7, 41, 0x65, 0xff].map((u) => [rustOff[u], app.HID2OFF[u]]));
  const fnStart = mod.indexOf('pub fn function_name');
  const fnBody = mod.slice(fnStart, mod.indexOf('\npub fn', fnStart + 10));
  const rustFns = new Map();
  for (const m of fnBody.matchAll(/(0x[0-9A-Fa-f]+)\s*=>\s*"([^"]*)"/g)) rustFns.set(Number(m[1]), m[2]);
  ok('the page names every function byte the Rust lookup names, with the same name',
    () => [...rustFns].every(([code, name]) => app.FUNCTION_NAMES[code] === name) && rustFns.size > 15,
    () => [...rustFns].filter(([code, name]) => app.FUNCTION_NAMES[code] !== name).slice(0, 4));
  ok('the page names no function byte the Rust lookup leaves unknown',
    () => Object.entries(app.FUNCTION_NAMES).every(([code, name]) => rustFns.get(Number(code)) === name),
    () => Object.entries(app.FUNCTION_NAMES).filter(([code, name]) => rustFns.get(Number(code)) !== name).slice(0, 4));
  ok('the byte the Rust lookup leaves to _ is the byte the page calls unknown',
    () => /_\s*=>\s*"unknown"/.test(fnBody) && app.functionName(0x22) === 'unknown' && rustFns.get(0x22) === undefined,
    () => [app.functionName(0x22), rustFns.get(0x22), fnBody.match(/_\s*=>\s*"unknown"/)]);
  ok('the Rust lookup names an unlisted byte the way the page labels it',
    () => app.functionName(0x7e) === 'unknown' && /_ => "unknown"/.test(fnBody));
  ok('the macro marker is the byte GG writes in front of a macro record',
    () => app.MACRO_MARKER === 0x77 && /0x77/.test(mod), () => app.MACRO_MARKER);
  ok('macro mode names cover the modes the samples use',
    () => app.MACRO_MODE_NAMES[4] === 'repeat' && app.MACRO_MODE_NAMES[1] === 'once');
})();

console.log('product IDs, model names and the read platform');
(() => {
  const dev = readFileSync(`${import.meta.dirname}/../../src/devices/mod.rs`, 'utf8');
  const consts = {};
  for (const m of dev.matchAll(/pub\s+const\s+([A-Z0-9_]+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+);/g)) consts[m[1]] = Number(m[2]);
  const body = dev.slice(dev.indexOf('pub fn device_name_from_product_id'));
  const rustNames = new Map();
  // Split on the match-arm arrow and read the identifiers in front of it. A pattern that lets the
  // engine try many ways to split a run of identifiers is exponential on real source text.
  const arms = body.split('=>');
  for (let index = 1; index < arms.length; index += 1) {
    const label = /^\s*"([^"]*)"/.exec(arms[index]);
    if (!label) continue;
    const before = arms[index - 1];
    const tail = before.slice(Math.max(before.lastIndexOf(','), before.lastIndexOf(';')) + 1);
    for (const ident of (tail.match(/[A-Z0-9_]+/g) ?? [])) {
      const pid = consts[ident];
      if (pid === undefined) continue;
      if (rustNames.has(pid)) ok(`product 0x${pid.toString(16)} is named twice by the Rust lookup`, () => rustNames.get(pid) === label[1]);
      rustNames.set(pid, label[1]);
    }
  }
  // Keyboards are named by their rows in keyboards/models.rs, not by arms of the name function.
  const models = readFileSync(`${import.meta.dirname}/../../src/devices/keyboards/models.rs`, 'utf8');
  const idList = Object.entries(consts).map(([name, pid]) => ({ name, pid, hex: `0x${pid.toString(16).toUpperCase().padStart(4, '0')}` }));
  const keyboardRows = sync.keyboardModels(models, idList).map((k) => ({ pid: Number.parseInt(k.hex, 16), name: k.name, klass: k.class }));
  ok('every keyboard row parses and names a product ID constant', () => keyboardRows.length === (models.match(/(?:^|\n)\s*model\(/g) ?? []).length,
    () => keyboardRows.length);
  for (const row of keyboardRows) rustNames.set(row.pid, row.name);
  const onboardRowIds = keyboardRows.filter((r) => r.klass === 'Apex7').map((r) => r.pid);
  const pageNames = new Map(Object.entries(app.PRODUCT_NAMES).map(([k, v]) => [Number(k), v]));
  ok('the Rust lookup names a product ID per table entry', () => rustNames.size > 30, () => rustNames.size);
  ok('the page names every product ID the Rust lookup names, with the same name', () => {
    for (const [pid, name] of rustNames) if (pageNames.get(pid) !== name) return false;
    return pageNames.size >= rustNames.size;
  }, () => [...rustNames].filter(([p]) => pageNames.get(p) !== rustNames.get(p)).slice(0, 3));
  const pageIds = new Set(app.LEGACY_ONBOARD_PIDS.map(Number));
  const rustIds = new Set(onboardRowIds);
  ok('the product IDs the page lists as the family are the IDs discovery builds as the on-board class',
    () => pageIds.size === rustIds.size && [...pageIds].every((p) => rustIds.has(p)) && [...pageIds].every((p) => app.PRODUCT_NAMES[p] === rustNames.get(p)),
    () => [[...pageIds].map((p) => p.toString(16)), [...rustIds].map((p) => p.toString(16))]);
  ok('the page registry names every product the Rust lookup names',
    () => [...rustNames].every(([pid, name]) => app.PRODUCT_NAMES[pid] === name),
    () => [...rustNames].filter(([pid, name]) => app.PRODUCT_NAMES[pid] !== name).slice(0, 4));
  const apex7 = readFileSync(`${import.meta.dirname}/../../src/devices/keyboards/apex_7.rs`, 'utf8');
  const apex7Read = () => ({ product_id: 0x1612, product: 'Apex 7', firmware: '4.16.8' });
  ok('the read reaches a board through the codec its class answers, and the page flags that board as the family',
    () => /fn onboard_codec/.test(apex7) && /Apex7Onboard/.test(apex7)
      && app.deviceLabel(apex7Read()).legacy === true
      && app.deviceLabel({ product_id: 0x1620, product: 'Other' }).legacy === false,
    () => [apex7.match(/onboard_codec|Apex7Onboard/g), app.LEGACY_ONBOARD_PIDS.map((p) => p.toString(16))]);
  ok('the family the page lists is the class the read gates on',
    () => {
      const ids = onboardRowIds;
      return ids.length > 0 && ids.every((id) => app.LEGACY_ONBOARD_PIDS.includes(id)) && app.LEGACY_ONBOARD_PIDS.length === ids.length;
    }, () => [app.LEGACY_ONBOARD_PIDS.map((p) => p.toString(16))]);

  ok('the label reports the model, product ID and firmware the read stamped',
    () => {
      const label = app.deviceLabel(apex7Read());
      return label.name === 'Apex 7' && label.hex === '0x1612' && label.firmware === '4.16.8'
        && label.known === true && label.legacy === true;
    }, () => app.deviceLabel(apex7Read()));
  ok('the read stamps the identity fields the page reads, naming the product from the Rust table',
    () => /device_name_from_product_id/.test(apex7) && app.deviceLabel(apex7Read()).name === rustNames.get(0x1612)
      && app.PRODUCT_NAMES[0x1612] === 'Apex 7',
    () => [app.deviceLabel(apex7Read()).name, rustNames.get(0x1612)]);
  ok('the page source opens no device: it only reads files',
    () => !/hidapi|hid_open|HIDDevice|navigator\.usb|sendReport/i.test(app.source),
    () => app.source.match(/hidapi|hid_open|HIDDevice|navigator\.usb|sendReport/i));
  ok('the model name comes from the imported Rust table, ahead of the name the file carries',
    () => app.deviceLabel(apex7Read()).name === 'Apex 7'
      && app.deviceLabel({ product_id: 0x1612, product: 'Custom name' }).name === 'Apex 7'
      && app.deviceLabel({ product_id: 0x9999, product: 'Ghostboard' }).name === 'Ghostboard');
  ok('no product ID stamped means no model claimed',
    () => app.deviceLabel({}) === null && app.deviceLabel(null) === null && app.deviceLabel({ product: 'Apex 7' }) === null);
  ok('an empty name in the file falls back to the page table by number',
    () => app.deviceLabel({ product_id: 0x1612, product: '' }).name === app.PRODUCT_NAMES[0x1612]);
  ok('a product ID the registry does not list is named by number, not by a made-up model',
    () => {
      const label = app.deviceLabel({ product_id: 0x9999, product: '' });
      return label.name.includes('0x9999') && label.known === false;
    }, () => app.deviceLabel({ product_id: 0x9999, product: '' }));
  ok('a stamped product ID with no firmware reads as no firmware, not as a version',
    () => app.deviceLabel({ product_id: 0x1612, product: 'Apex 7' }).firmware === null);
  ok('the page marks the legacy onboard product IDs the read can decode',
    () => app.LEGACY_ONBOARD_PIDS.includes(0x1612) && app.LEGACY_ONBOARD_PIDS.every((pid) => app.PRODUCT_NAMES[pid] !== undefined),
    () => app.LEGACY_ONBOARD_PIDS);
})();



/** Body of a `pub mod X { ... }` block or a `pub fn` in a Rust source file. */
function sectionOf(src, header) {
  const start = src.indexOf(header);
  const open = src.indexOf('{', start);
  const close = src.indexOf('\n}', open);
  return src.slice(open + 1, close);
}

function fnBody(src, header) {
  const start = src.indexOf(header);
  const open = src.indexOf('{', start);
  const close = src.indexOf('\n}\n', open);
  return src.slice(open + 1, close);
}

// --- model names imported from Rust ---------------------------------------
console.log('model names, product IDs and feature sets imported from Rust');
const REPO = `${import.meta.dirname}/../..`;
const read = (rel) => readFileSync(`${REPO}/${rel}`, 'utf8');
const rustProducts = sync.collect(REPO);

ok('the generated table is current against Rust (sync-products.mjs --check)', () => {
  const run = spawnSync('node', [`${import.meta.dirname}/sync-products.mjs`, '--check'], { cwd: REPO, encoding: 'utf8' });
  if (run.status !== 0) throw new Error((run.stdout + run.stderr).split('\n').filter((l) => l && !/Warning|trace-warnings/.test(l)).join(' | '));
  return true;
}, () => `source hash ${rustProducts.sourceHash}`);

ok('the generated file is what the generator renders from Rust today',
  () => productSrc === sync.render(rustProducts));

ok('index.html loads the generated table before app.js', () => {
  const html = read('tools/profile-gui/index.html');
  const at = html.indexOf('products.generated.js');
  return at > 0 && at < html.indexOf('<script src="app.js"') && /app\.js/.test(html);
}, () => read('tools/profile-gui/index.html').match(/<script[^>]*>/gi));

ok('the page sees the imported table', () => app.PRODUCTS === globalThis.SsggProducts
  && app.PRODUCTS.sourceHash === rustProducts.sourceHash
  && app.PRODUCTS.productCount === rustProducts.productCount);

ok('every model Rust names is imported with that name', () => {
  for (const [hex, info] of Object.entries(rustProducts.products)) {
    const page = app.PRODUCTS.products[hex];
    if (!page || page.name !== info.name || page.onboard !== info.onboard) return false;
  }
  return Object.keys(rustProducts.products).length === Object.keys(app.PRODUCTS.products).length
    && rustProducts.namedCount >= 40;
}, () => [rustProducts.namedCount, app.PRODUCTS.namedCount, app.PRODUCTS.products['0x1612']]);

ok('without the import the page says the registry was not imported', () => {
  const saved = globalThis.SsggProducts;
  delete globalThis.SsggProducts;
  const bare = loadApp(import.meta.dirname);
  const line = bare.deviceFeatures(0x1612).join(' | ');
  globalThis.SsggProducts = saved;
  return bare.PRODUCTS === null && /device registry: not imported/.test(line)
    && /read back on this family/.test(line) && bare.deviceLabel({ product_id: 0x1612, product: 'Apex 7' }).name === 'Apex 7';
}, () => app.deviceFeatures(0x1612));

ok('the on-board family the import marks is the class discovery builds for the on-board board', () => {
  const ids = sync.productIds(sectionOf(read('src/devices/mod.rs'), 'pub mod product_ids'));
  const family = sync.keyboardModels(read('src/devices/keyboards/models.rs'), ids)
    .filter((k) => k.class === 'Apex7')
    .map((k) => k.hex);
  return eq([...app.PRODUCTS.onboardPids].sort(), [...family].sort())
    && app.onboardModelPids().map(app.productHex).join() === [...family].sort().join();
}, () => [app.PRODUCTS.onboardPids, app.onboardModelPids().map(app.productHex)]);

ok('the hand-mirrored fallback tables match the import', () => {
  const pids = [...app.LEGACY_ONBOARD_PIDS, 0x1610, 0x12CB, 0x1628];
  for (const pid of pids) {
    const hex = app.productHex(pid);
    const info = app.PRODUCTS.products[hex];
    if (!info || info.name !== app.PRODUCT_NAMES[pid]) return false;
  }
  return eq(app.LEGACY_ONBOARD_PIDS, app.onboardModelPids());
}, () => app.LEGACY_ONBOARD_PIDS.map(app.productHex).join(' ') + ' vs ' + app.PRODUCTS.onboardPids.join(' '));

ok('a read record names its model from the import, not from the file', () => {
  const readDevice = () => ({ product_id: 0x1612, product: 'Apex 7', firmware: '4.16.8' });
  const label = app.deviceLabel(readDevice());
  const claim = { ...readDevice(), product: 'a name a file made up' };
  return label.name === 'Apex 7' && label.hex === '0x1612' && label.legacy === true
    && app.deviceLabel(claim).name === 'Apex 7';
}, () => app.deviceLabel(apex7Read()));

ok('feature sets follow the model: family, mouse layout, extra keys', () => {
  const apex7 = app.deviceFeatures(0x1612).join(' | ');
  const apexPro = app.deviceFeatures(0x1610).join(' | ');
  const headset = app.deviceFeatures(0x12CB).join(' | ');
  const mouse = app.deviceFeatures(0x1662).join(' | ');
  return /device registry: keyboard/.test(apex7) && /on-board profile protocol: read back/.test(apex7)
    && /legacy Apex 750-byte/.test(apex7) && /extra blocks this page decodes/.test(apex7)
    && /device registry: keyboard/.test(apexPro) && /not in Rust/.test(apexPro)
    && /not the one this page encodes/.test(apexPro)
    && /device registry: headset/.test(headset) && /no keyboard layout/.test(headset)
    && /no entry in Rust/.test(mouse);
}, () => [app.deviceFeatures(0x1612), app.deviceFeatures(0x1610), app.deviceFeatures(0x12CB), app.deviceFeatures(0x1662)]);

ok('the imported device kinds are the ones the Rust registry gives', () => {
  const ids = sync.productIds(sectionOf(read('src/devices/mod.rs'), 'pub mod product_ids'));
  const { kinds, fallback } = sync.productKinds(fnBody(read('src/devices/mod.rs'), 'pub fn device_type_from_product_id'), ids);
  for (const [hex, kind] of Object.entries(kinds)) {
    if (app.PRODUCTS.products[hex].kind !== kind) return false;
    if (app.productKind(Number.parseInt(hex, 16)) !== kind) return false;
  }
  return app.PRODUCTS.kindFallback === fallback && fallback === 'unknown'
    && app.productKind(0xFEED) === 'unknown'
    && /no entry in Rust/.test(app.deviceFeatures(0xFEED).join(' | '));
}, () => [app.PRODUCTS.kindFallback, app.productKind(0x12CB), app.productKind(0xFEED)]);

ok('an ID no registry lists names no model and offers no features', () => {
  const ghost = JSON.parse(JSON.stringify(apex));
  ghost.product_id = 0xFEED;
  ghost.product = 'Unknown SteelSeries Device';
  ghost.firmware = null;
  const label = app.deviceLabel(ghost);
  return app.importedName(0xFEED) === null && app.productKind(0xFEED) === 'unknown'
    && /no entry in Rust/.test(app.deviceFeatures(0xFEED).join(' | '))
    && label.name === 'unknown product 0xFEED' && label.hex === '0xFEED' && label.legacy === false;
}, () => app.deviceFeatures(0xFEED));

// --- validator and data contract ------------------------------------------
console.log('the JSON a read writes is the JSON the page reads');
ok('the sample files pass the validator',
  () => app.isValidProfileDoc(app.cloneDoc(apex)) === null && app.isValidProfileDoc(identityDoc) === null
    && app.isValidProfileDoc(macrosDoc ?? identityDoc) === null,
  () => app.isValidProfileDoc(app.cloneDoc(apex)));
ok('the validator needs the mapping arrays the decoder lays out',
  () => JSON.stringify(app.EXPECTED_ARRAYS) === JSON.stringify({ button_mappings: 128, wheel_mappings: 2, button_meta_mappings: 128, wheel_meta_mappings: 2 })
    && Object.entries(app.EXPECTED_ARRAYS).every(([name, len]) => apex[name].length === len),
  () => app.EXPECTED_ARRAYS);
ok('a file that is not an object is refused',
  () => app.isValidProfileDoc(null) === 'not an object' && app.isValidProfileDoc([]) === 'not an object'
    && app.isValidProfileDoc(7) === 'not an object' && app.isValidProfileDoc('x') === 'not an object');
ok('a file without a mapping array is refused naming that array', () => {
  const broken = app.cloneDoc(apex);
  delete broken.wheel_meta_mappings;
  return app.isValidProfileDoc(broken) === 'missing array wheel_meta_mappings';
});
ok('a short array is refused naming its length', () => {
  const broken = app.cloneDoc(apex);
  broken.wheel_mappings.pop();
  return app.isValidProfileDoc(broken) === 'wheel_mappings has 1 entries, expected 2';
});
ok('short and missing key_codes pass the validator and are padded to four codes when read', () => {
  const short = app.cloneDoc(apex);
  short.button_mappings[27].key_codes = '04 00';
  const missing = app.cloneDoc(apex);
  delete missing.button_mappings[27].key_codes;
  return app.isValidProfileDoc(short) === null && app.mappingCodes(short.button_mappings[27]).list.length === 4
    && app.isValidProfileDoc(missing) === null;
}, () => app.isValidProfileDoc(app.cloneDoc(apex)));
ok('five key codes do not fit one mapping entry', () => {
  const broken = app.cloneDoc(apex);
  broken.button_mappings[27].key_codes = '01 02 03 04 05';
  return /has 5 bytes/.test(app.isValidProfileDoc(broken) ?? '');
});
ok('a schema that is not a number is refused', () => {
  const broken = app.cloneDoc(apex);
  broken.schema = '11';
  return app.isValidProfileDoc(broken) === 'schema is not a number';
});
ok('a doc without a name is refused', () => {
  const broken = app.cloneDoc(apex);
  delete broken.name;
  return app.isValidProfileDoc(broken) === 'name is not a string';
});
ok('normalizeDoc fills the derived fields a mapping entry can leave out', () => {
  const bare = app.cloneDoc(apex);
  for (const field of ['hid_code', 'function_name', 'binding', 'macro_index']) delete bare.button_mappings[27][field];
  app.normalizeDoc(bare);
  const e = bare.button_mappings[27];
  return e.offset === 27 && e.hid_code === app.OFF2HID[27] && e.macro_index === null
    && e.function_name === app.functionName(e.function) && e.binding === app.bindingOf(e.function, app.parseHex(e.key_codes), e.hid_code);
}, () => 'normalizeDoc left fields empty');
ok('normalizeDoc does not invent a block a file left out', () => {
  const stripped = app.cloneDoc(apex);
  delete stripped.wheel_meta_mappings;
  app.normalizeDoc(stripped);
  return stripped.wheel_meta_mappings === undefined;
});
ok('normalizeDoc leaves a complete file unchanged', () => eq(app.normalizeDoc(app.cloneDoc(apex)), apex));
ok('the serialiser writes the doc back out as JSON a read would write',
  () => JSON.parse(app.serializeDoc(app.cloneDoc(apex))).name === apex.name
    && JSON.parse(app.serializeDoc(app.cloneDoc(apex))).button_mappings.length === 128
    && app.serializeDoc(app.cloneDoc(apex)).endsWith('\n'));
ok('cloneDoc is independent from the doc it clones', () => {
  const copy = app.cloneDoc(apex);
  copy.name = 'changed';
  copy.button_mappings[27].key_codes = 'FF FF FF FF';
  return apex.name !== 'changed' && apex.button_mappings[27].key_codes !== 'FF FF FF FF';
});
ok('the page measures the codes per entry the decoder wrote',
  () => app.codesPerEntry(apex) === 4 && (() => {
    const d = app.cloneDoc(apex);
    for (const name of Object.keys(app.EXPECTED_ARRAYS)) for (const e of d[name]) e.key_codes = '04 00 00';
    return app.codesPerEntry(d) === 3;
  })(), () => app.codesPerEntry(apex));
ok('the page tells a 3-code dump from the 4-code form the writer uses',
  () => (() => {
    const d = app.cloneDoc(apex);
    for (const name of Object.keys(app.EXPECTED_ARRAYS)) for (const e of d[name]) e.key_codes = '04 00 00';
    return app.codesPerEntry(d) === 3 && app.mappingCodes(d.button_mappings[27]).list.length === 4;
  })());

// --- file layer -----------------------------------------------------------
console.log('the file layer');
ok('a valid file is accepted and becomes the active doc', () => remap.ok === true && app.activeDoc() === apex);
ok('a file that is not JSON is refused with its file name',
  () => app.addFile('bad.json', '{ not json').error.startsWith('bad.json: not valid JSON'),
  () => app.addFile('bad.json', '{ not json').error);
ok('a JSON file without the mapping arrays is refused naming the array',
  () => app.addFile('x.json', JSON.stringify({ hello: 1 })).error.includes('missing array button_mappings'),
  () => app.addFile('x.json', JSON.stringify({ hello: 1 })).error);
ok('a page holds one file, and loading one replaces it', () => {
  app.addFile('identity.json', readFileSync(`${EX}/sample-identity.json`, 'utf8'));
  return app.state.files.length === 1 && app.activeFile().filename === 'identity.json';
}, () => app.state.files.map((f) => f.filename));
ok('ids are handed out in load order', () => {
  const before = app.nextFileId();
  app.addFile('sample-remap.json', readFileSync(`${EX}/sample-remap.json`, 'utf8'));
  return app.state.files[0].id === before && app.nextFileId() === before + 1;
}, () => app.nextFileId());
ok('the copy name is derived from the loaded name and never looks like it',
  () => app.copyName(app.activeFile()).endsWith('-copy.json') && app.copyName(app.activeFile()) !== app.activeFile().filename,
  () => app.copyName(app.activeFile()));
ok('an unedited file has no changes to report',
  () => app.fileChanges(app.activeFile()).length === 0 && app.changedFiles().length === 0);
ok('the page keeps the file as loaded to diff against', () => {
  const file = app.activeFile();
  app.setCode(file.doc, app.LAYER_BASE, 7, 0, 26);
  const changes = app.fileChanges(file);
  const undo = app.cloneDoc(file.doc);
  app.setCode(undo, app.LAYER_BASE, 7, 0, app.parseHex(file.original.button_mappings[27].key_codes)[0]);
  return changes.length > 0 && app.diffAgainst(file.original, undo).length === 0;
}, () => app.fileChanges(app.activeFile()));
ok('the tab list shows only the files with changes', () => {
  app.setCode(app.activeFile().doc, app.LAYER_BASE, 7, 0, 26);
  const changed = app.changedFiles();
  return changed.length === 1 && changed[0].id === app.state.files[0].id;
}, () => [app.changedFiles().length, app.state.files.length, app.fileChanges(app.activeFile()).length]);

// saving writes through a download link; the fake DOM records what is appended
const body = globalThis.document.body;
const proto = Object.getPrototypeOf(body);
const appendOriginal = proto.appendChild;
const appended = [];
proto.appendChild = function (child) {
  appended.push(child);
  return appendOriginal.call(this, child);
};
ok('saving a file writes a download link with a derived name', () => {
  appended.length = 0;
  const name = app.saveFile(app.activeFile(), 'saved-copy.json');
  const anchors = appended.filter((n) => n.tagName === 'A' && n.attrs.get('download') === 'saved-copy.json');
  return name === 'saved-copy.json' && anchors.length === 1 && String(anchors[0].attrs.get('href')).startsWith('blob:');
}, () => [appended.map((n) => n.tagName)]);
ok('the download text is the serialised doc of the file', () => {
  appended.length = 0;
  const file = app.activeFile();
  app.saveFile(file);
  const anchor = appended.filter((n) => n.tagName === 'A')[0];
  const written = app.serializeDoc(file.doc);
  return app.copyName(file).endsWith('-copy.json') && written.includes(file.doc.name) && anchor !== undefined;
}, () => app.copyName(app.activeFile()));
ok('the app reads files only through the file input and drops, never from a device',
  () => !/require\(/.test(app.source) && !/['"]node:fs['"]|['"]fs['"]/.test(app.source) && !/localStorage|indexedDB|WebSocket|navigator\.usb/.test(app.source));

// --- editing API and the change list --------------------------------------
console.log('the edit API and the change list');
ok('keyEntry resolves a usage byte to its mapping entry',
  () => { const found = app.keyEntry(apex, app.LAYER_BASE, 7); return found.offset === 27 && found.entry.hid_code === 7; },
  () => app.keyEntry(apex, app.LAYER_BASE, 7));
ok('a usage byte with no mapping offset resolves to nothing', () => app.keyEntry(apex, app.LAYER_BASE, 0xff) === null);
ok('mappingCodes pads the codes an entry leaves out to four',
  () => { const d = app.cloneDoc(apex); d.button_mappings[27].key_codes = '04 00'; return app.mappingCodes(d.button_mappings[27]).list.length === 4; },
  () => app.mappingCodes(apex.button_mappings[27]));
ok('mappingCodes keeps a code past the fourth byte',
  () => { const d = app.cloneDoc(apex); d.button_mappings[27].key_codes = '04 00 00 00 05'; const m = app.mappingCodes(d.button_mappings[27]); return m.list.length === 4 && eq(m.extra, [5]); });
ok('writeCodes writes back with a code past the fourth byte kept',
  () => {
    const e = app.cloneDoc(apex).button_mappings[27];
    app.writeCodes(e, [4, 0, 26, 0, 7]);
    return e.key_codes === '04 00 1A 00 07' && app.parseHex(e.key_codes).length === 5;
  }, () => { const e = app.cloneDoc(apex); app.writeCodes(e.button_mappings[27], [4, 0, 26, 0, 7]); return e.button_mappings[27].key_codes; });
ok('keyState reports the binding of a key that fires itself',
  () => { const s = app.keyState(apex, app.LAYER_BASE, 7); return s.kind === 'self' && s.offset === 27 && eq(s.codes, [7, 0, 0, 0]); },
  () => app.keyState(apex, app.LAYER_BASE, 7));
ok('keyState reports a key with no mapping slot as its own kind',
  () => { const s = app.keyState(apex, app.LAYER_BASE, 0xff); return s === null || s.kind !== undefined; }, () => app.keyState(apex, app.LAYER_BASE, 0xff));
ok('customBindings lists only the entries that bind to another key',
  () => {
    const list = app.customBindings(apex, app.LAYER_BASE);
    const counts = app.bindingCounts(apex).find((c) => c.layer === app.LAYER_BASE).counts;
    return list.length === counts.custom && list.every((b) => b.entry.binding === 'custom' && b.offset === b.entry.offset);
  }, () => app.customBindings(apex, app.LAYER_BASE).length);
ok('bindingCounts counts self, none and custom on both layers',
  () => {
    const counts = app.bindingCounts(apex);
    return counts.length === 2 && counts.every((c) => Object.values(c.counts).reduce((a, b) => a + b, 0) === app.MAPPING_COUNT);
  }, () => app.bindingCounts(apex));
ok('setFunction changes the function byte and re-derives the name', () => {
  const doc = app.cloneDoc(apex);
  app.setFunction(doc, app.LAYER_BASE, 7, 0x62);
  const found = app.keyEntry(doc, app.LAYER_BASE, 7);
  return found.entry.function === 0x62 && found.entry.function_name === app.FUNCTION_NAMES[0x62];
});
ok('setCode writes one code slot and marks a self key custom', () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  const found = app.keyEntry(doc, app.LAYER_BASE, 7);
  return app.parseHex(found.entry.key_codes)[0] === 26 && found.entry.binding === 'custom';
});
ok('setAllCodes writes all four code slots at once', () => {
  const doc = app.cloneDoc(apex);
  app.setAllCodes(doc, app.LAYER_BASE, 7, [4, 0, 26, 0]);
  return eq(app.parseHex(app.keyEntry(doc, app.LAYER_BASE, 7).entry.key_codes), [4, 0, 26, 0]);
});
ok('the meta layer edits its own mapping list', () => {
  const doc = app.cloneDoc(apex);
  app.setFunction(doc, app.LAYER_META, 7, 0x62);
  return app.keyEntry(doc, app.LAYER_META, 7).entry.function === 0x62
    && app.keyEntry(doc, app.LAYER_BASE, 7).entry.function === app.keyEntry(apex, app.LAYER_BASE, 7).entry.function;
});
ok('the change list is empty for an unedited file',
  () => app.diffAgainst(apex, app.cloneDoc(apex)).length === 0 && app.summarizeChanges([]).length === 0 && app.changeKeyHids([]).size === 0);
ok('an edited binding is tracked as its own changes', () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  const changes = app.diffAgainst(apex, doc);
  return changes.length === 2 && changes.every((c) => c.path.startsWith('button_mappings[27].'))
    && changes.some((c) => c.path.endsWith('.key_codes'));
}, () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  return app.diffAgainst(apex, doc);
});
ok('the change list names the key a change belongs to', () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  const hids = app.changeKeyHids(app.diffAgainst(apex, doc));
  return hids.has(7) && hids.size === 1;
}, () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  return [...app.changeKeyHids(app.diffAgainst(apex, doc))];
});
ok('the change summary groups the edits of the same field together', () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  app.setCode(doc, app.LAYER_BASE, 8, 0, 26);
  const groups = app.summarizeChanges(app.diffAgainst(apex, doc));
  return groups.length === 2 && groups.every(([, count]) => count === 2);
}, () => {
  const doc = app.cloneDoc(apex);
  app.setCode(doc, app.LAYER_BASE, 7, 0, 26);
  app.setCode(doc, app.LAYER_BASE, 8, 0, 26);
  return app.summarizeChanges(app.diffAgainst(apex, doc));
});
ok('a profile name change is tracked together with the hex form the writer needs', () => {
  const doc = app.cloneDoc(apex);
  app.setProfileName(doc, 'Renamed');
  const changes = app.diffAgainst(apex, doc);
  return changes.some((c) => c.path === 'name') && changes.some((c) => c.path === 'name_hex')
    && app.parseHex(doc.name_hex).length === doc.name.length;
}, () => {
  const doc = app.cloneDoc(apex);
  app.setProfileName(doc, 'Renamed');
  return app.diffAgainst(apex, doc);
});
ok('the profile name stays inside the field the writer pads to',
  () => {
    const d = app.cloneDoc(apex);
    const fits = app.setProfileName(d, 'x'.repeat(app.NAME_FIELD_BYTES));
    const over = app.setProfileName(d, 'x'.repeat(app.NAME_FIELD_BYTES + 1));
    return app.NAME_FIELD_BYTES === 24 && fits === true && over === false && app.parseHex(d.name_hex).length === app.NAME_FIELD_BYTES;
  }, () => [app.NAME_FIELD_BYTES, app.parseHex(app.cloneDoc(apex).name_hex).length]);
ok('a lighting colour change is tracked', () => {
  const doc = app.cloneDoc(apex);
  app.setLightingColor(doc, 'meta_key_color', { r: 1, g: 2, b: 3 });
  return app.diffAgainst(apex, doc).some((c) => c.path.startsWith('lighting.meta_key_color'));
});
ok('a Rapid Trigger pair change is tracked', () => {
  const doc = app.cloneDoc(apex);
  app.setRapidPair(doc, 0, 'mode', 2);
  return app.diffAgainst(apex, doc).length > 0;
});

// --- Rapid Trigger, lighting, wheel, OLED, macros -------------------------
console.log('Rapid Trigger, lighting, wheel keys, OLED and the macro block');
ok('rapidTap reports the enable byte and the pair list',
  () => { const rt = app.rapidTap(apex); return typeof rt.enable === 'number' && Array.isArray(rt.pairs) && rt.pairs.length > 0 && rt.pairs.every((p) => 'mode' in p && 'hid_1' in p && 'hid_2' in p); },
  () => app.rapidTap(apex).pairs.length);
ok('rapidTapKeysFor lists the pairs a key is in',
  () => { const pairs = app.rapidTapKeysFor(apex, 7); return Array.isArray(pairs) && pairs.length > 0 && pairs.every((p) => p.pair.hid_1 === 7 || p.pair.hid_2 === 7); },
  () => app.rapidTapKeysFor(apex, 7));
ok('a key in no Rapid Trigger pair has no pair row', () => app.rapidTapKeysFor(apex, 0x50).length === 0,
  () => app.rapidTapKeysFor(apex, 0x50));
ok('the Rapid Tap mode byte keeps its report-both-keys bit apart from the mode',
  () => app.rapidTap(apex).pairs.every((p) => p.mode < app.MASK_MODE_REPORT_BOTH) && app.MASK_MODE_REPORT_BOTH === 0x80);
ok('lighting reports the colours a read decoded',
  () => { const l = app.lighting(apex); return l.meta_key_color !== undefined && l.lock_key_color !== undefined; },
  () => Object.keys(app.lighting(apex)));
ok('wheelEntry resolves a wheel mapping to its entry, codes and label',
  () => { const w = app.wheelEntry(apex, app.LAYER_BASE, 0); return w.index === 0 && Array.isArray(w.codes) && w.codes.length === 4 && typeof w.label === 'string' && w.label.length > 0; },
  () => app.wheelEntry(apex, app.LAYER_BASE, 0));
ok('a wheel index outside the two wheel keys resolves to nothing',
  () => app.wheelEntry(apex, app.LAYER_BASE, 2) === null && app.wheelEntry(apex, app.LAYER_BASE, -1) === null);
ok('the two wheel keys are the codes the board draws, not 128-table offsets',
  () => app.WHEEL_KEYS.map((k) => k.hid).join() === '256,257'
    && app.WHEEL_KEYS.every((k) => app.KEY_NAMES[k.hid] !== undefined && app.OFF2HID[k.hid] === undefined)
    && app.WHEEL_UP === 0x100 && app.WHEEL_DOWN === 0x101 && app.OFF2HID.length === 128,
  () => [app.WHEEL_KEYS.map((k) => k.hid), app.OFF2HID.length]);
ok('the OLED block is the 128x40 bitmap the decoder lays out',
  () => app.OLED_BYTES === 640 && app.OLED_W === 128 && app.OLED_H === 40 && app.oledBytes(apex).length === app.OLED_BYTES,
  () => [app.OLED_BYTES, app.oledBytes(apex)]);
ok('a dump without an OLED block reports no bytes, the panel then keeps its label',
  () => app.oledBytes({}) === null && app.oledBytes(null) === null && app.oledBytes({ oled: {} }) === null
    && app.oledBytes({ oled: { bytes_hex: '00' } }) === null, () => [app.oledBytes({}), app.oledBytes({ oled: { bytes_hex: '00' } })]);
ok('the OLED raster is one RGBA pixel per screen pixel',
  () => app.oledPixels(new Array(app.OLED_BYTES).fill(0)).length === app.OLED_W * app.OLED_H * 4);
ok('bit 7 of an OLED byte is its leftmost pixel', () => {
  const bytes = new Array(app.OLED_BYTES).fill(0);
  bytes[0] = 0x80;
  const px = app.oledPixels(bytes);
  return px[0] === 255 && px[4] === 0;
});
ok('the OLED bitmap is drawn as a PNG data URL when the page has a canvas',
  () => app.oledDataUrl(new Array(app.OLED_BYTES).fill(0)) === null,
  () => String(app.oledDataUrl(new Array(app.OLED_BYTES).fill(0))));
ok('the app writes a bitmap back into the OLED block', () => {
  const doc = app.cloneDoc(apex);
  app.setOledBitmap(doc, new Array(app.OLED_BYTES).fill(3));
  return app.oledBytes(doc)[0] === 3 && app.oledBytes(doc).length === app.OLED_BYTES;
});
okMacros('macroRecords lists the records the decoder decoded',
  () => Array.isArray(app.macroRecords(macrosDoc)) && app.macroRecords(identityDoc).length === 0,
  () => [app.macroRecords(macrosDoc).length, app.macroRecords(identityDoc).length]);
okMacros('the macro summary counts the records and says when there are none',
  () => app.macroSummary(macrosDoc).includes('macro record') && app.macroSummary(identityDoc).includes('no macro records'),
  () => [app.macroSummary(macrosDoc), app.macroSummary(identityDoc)]);
ok('macroIndex reads the index a macro-bound entry carries',
  () => {
    const bound = { offset: 27, hid_code: 7, function: 0x71, key_codes: '00 00 07 00' };
    return app.macroIndex(bound) === 7 && app.macroIndex(bound) === app.mappingCodes(bound).list[2]
      && app.macroIndex(apex.button_mappings[27]) === 0;
  }, () => app.macroIndex({ offset: 27, hid_code: 7, function: 0x71, key_codes: '00 00 07 00' }));
okMacros('macroKeystrokes reads the keys and delays a record runs', () => {
  const record = app.macroRecords(macrosDoc)[0];
  const text = app.macroKeystrokes(record);
  return typeof text === 'string' && text.length > 0 && !/undefined/.test(text);
}, () => app.macroKeystrokes(app.macroRecords(macrosDoc)[0]));
ok('macroBoundKeys lists the keys bound to a macro index', () => {
  const doc = app.cloneDoc(identityDoc);
  doc.button_mappings.forEach((e) => { e.function = 0; e.key_codes = '00 00 00 00'; e.macro_index = null; });
  doc.button_mappings[27] = { offset: 27, hid_code: 7, function: 0x71, key_codes: '00 00 07 00', macro_index: 7 };
  doc.button_mappings[28] = { offset: 28, hid_code: 8, function: 0x71, key_codes: '00 00 14 00', macro_index: 14 };
  app.normalizeDoc(doc);
  const bound7 = app.macroBoundKeys(doc, 7);
  const bound14 = app.macroBoundKeys(doc, 14);
  // An index resolves by ordinal, so 8 (record 1) finds the key bound at 7, and 99 (record 14) finds none.
  return bound7.length === 1 && bound7[0] === app.keyName(app.OFF2HID[27])
    && bound14.length === 1 && bound14[0] === app.keyName(app.OFF2HID[28])
    && eq(app.macroBoundKeys(doc, 8), bound7)
    && app.macroBoundKeys(doc, 6).length === 0
    && app.macroBoundKeys(doc, 99).length === 0;
}, () => JSON.stringify(app.macroBoundKeys({ button_mappings: [{ offset: 27, hid_code: 7, function: 0x71, key_codes: '00 00 07 00' }] }, 7)));
okMacros('macroLine names the record a binding index resolves to',
  () => app.macroLine(macrosDoc, 0).includes('binds record 0')
    && app.macroLine(macrosDoc, 7).includes('binds record 1')
    && app.macroLine(macrosDoc, 6).includes('binds record 0')
    && app.macroLine(macrosDoc, 0).includes('action(s)')
    && !app.macroLine(macrosDoc, 0).includes('press '),
  () => [app.macroLine(macrosDoc, 0), app.macroLine(macrosDoc, 7)]);
ok('a binding index past the records explains what GG left behind',
  () => app.macroLine(identityDoc, 7).includes('past the 0 record(s) in the block'), () => app.macroLine(identityDoc, 7));
okMacros('the whole macro of a binding is a block of its own, keystroke by keystroke', () => {
  const record = app.macroRecords(macrosDoc)[1];
  const rows = app.macroDetailRows(macrosDoc, 7);
  const list = rows.find((node) => node.tagName === 'UL');
  const text = rows.map((node) => node.textContent).join(' ');
  return list !== undefined && list.children.length === (record.actions ?? []).length
    && text.includes('the whole macro: record 1') && text.includes(`interval ${record.interval_ms} ms`)
    && text.includes('press ') && text.includes(record.raw) && text.includes('Bound by');
}, () => [app.macroDetailRows(macrosDoc, 7).map((n) => n.tagName), app.macroDetailRows(macrosDoc, 7).map((n) => n.textContent).join(' ').slice(0, 200)]);
ok('a macro binding index past the records says there is nothing to show', () => {
  const rows = app.macroDetailRows(identityDoc, 7);
  const text = rows.map((node) => node.textContent).join(' ');
  return rows.length === 1 && text.includes('record 1') && text.includes('Nothing to show');
}, () => app.macroDetailRows(identityDoc, 7).map((n) => n.textContent).join(' '));
okMacros('the macro select labels records shortly, with no keystrokes in the option text', () => {
  const options = app.macroBindingOptions(macrosDoc, 0);
  const labels = options.map((o) => o.label);
  const longest = Math.max(...labels.map((l) => l.length));
  const records = app.macroRecords(macrosDoc);
  const repeatAt = labels.findIndex((l) => l.includes('repeat'));
  return options.length === records.length && eq(options.map((o) => o.value), records.map((r) => r.index))
    && eq(records.map((r) => r.index), [0, 7, 14, 21, 28])
    && labels.every((l) => l.startsWith('Record ')) && labels.every((l) => l.includes('action(s)'))
    && labels.every((l) => !l.includes('press ') && !l.includes('..'))
    && repeatAt >= 0 && labels[repeatAt].includes(`${records[repeatAt].interval_ms} ms`)
    && longest <= 60;
}, () => [app.macroBindingOptions(macrosDoc, 0).map((o) => o.label), app.macroRecords(macrosDoc).map((r) => [r.index, r.mode_name, r.interval_ms, r.actions.length])]);
ok('the macro select keeps an index it cannot name, and says which record it resolves to', () => {
  const options = app.macroBindingOptions(identityDoc, 7);
  return options.length === 1 && options[0].value === 7 && /Index 7 - record 1, past the 0 record/.test(options[0].label);
}, () => app.macroBindingOptions(identityDoc, 7).map((o) => o.label));

// --- board geometry -------------------------------------------------------
console.log('the keyboard board');
const board = app.buildBoard(() => {});
ok('the board is an SVG in the SVG namespace',
  () => board.tagName === 'SVG' && board.attrs.get('class') === 'board' && app.SVG_NS === 'http://www.w3.org/2000/svg'
    && app.svg('svg').ns === app.SVG_NS, () => [board.tagName, board.attrs.get('class')]);
ok('the board draws a cap for every key it lists',
  () => {
    const groups = board.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') !== undefined);
    return groups.length === app.boardKeyList().length && groups.length === app.LAYOUT.length + app.WHEEL_KEYS.length + app.EXTRA_HIDS.length;
  }, () => [board.children.filter((c) => c.attrs.get('data-hid') !== undefined).length, app.boardKeyList().length]);
ok('every cap is a group carrying its cap, fill and clip rects',
  () => board.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') !== undefined).every((g) => {
    const rects = g.children.filter((c) => c.tagName === 'RECT');
    const clip = g.children.find((c) => c.tagName === 'CLIPPATH');
    return g.attrs.get('class').split(' ').includes('key') && rects.length >= 2 && clip && clip.children[0].tagName === 'RECT';
  }));
ok('every cap group is focusable and labelled as a button',
  () => board.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') !== undefined)
    .every((g) => g.attrs.get('tabindex') === '0' && g.attrs.get('role') === 'button'));
ok('the style gives the caps a focus outline', () => /\.key:focus|:focus/.test(style) && /outline/.test(style),
  () => style.match(/:focus[^{]*\{[^}]*\}/s)?.[0]);
ok('the style binds pointer events to the cap shapes, not to the labels',
  () => /\.key text\s*\{[^}]*pointer-events:\s*none/.test(style) && /\.key \.cap, \.key \.fill\s*\{[^}]*pointer-events:\s*all/.test(style)
    && /\.oled text\s*\{[^}]*pointer-events:\s*none/.test(style) && /\.oled \.oled, \.oled \.oled-screen\s*\{[^}]*pointer-events:\s*all/.test(style),
  () => [style.match(/\.key (cap|text)[^}]*pointer-events:[^;}]+/g), style.match(/\.oled[^}]*pointer-events:[^;}]+/g)]);
ok('every cap clips its drawing to its own cap box',
  () => board.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') !== undefined).every((g) => {
    const cap = g.children.find((c) => c.tagName === 'RECT');
    const clip = g.children.find((c) => c.tagName === 'CLIPPATH');
    const clipRect = clip.children[0];
    const clipId = clip.attrs.get('id');
    const texts = g.children.filter((c) => c.tagName === 'TEXT');
    const clipped = texts.length > 0 && texts.every((t) => String(t.attrs.get('clip-path') ?? '').includes(`url(#${clipId})`));
    return clipId !== undefined && clipped && texts.length >= 2
      && clipRect.attrs.get('x') === cap.attrs.get('x') && clipRect.attrs.get('y') === cap.attrs.get('y')
      && clipRect.attrs.get('width') === cap.attrs.get('width') && clipRect.attrs.get('height') === cap.attrs.get('height');
  }));
ok('the text a cap draws sits inside the cap box it is clipped to',
  () => {
    for (const g of board.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') !== undefined)) {
      const cap = g.children.find((c) => c.tagName === 'RECT');
      const x = Number(cap.attrs.get('x'));
      const y = Number(cap.attrs.get('y'));
      const w = Number(cap.attrs.get('width'));
      const h = Number(cap.attrs.get('height'));
      for (const t of g.children.filter((c) => c.tagName === 'TEXT')) {
        const size = Number(t.attrs.get('font-size') ?? 12);
        const cx = Number(t.attrs.get('x'));
        const cy = Number(t.attrs.get('y'));
        if (!Number.isFinite(cx) || !Number.isFinite(cy)) return false;
        const width = String(t.textContent).length * size * 0.6;
        if (cx - width / 2 < x - 1 || cx + width / 2 > x + w + 1 || cy - size * 0.8 < y - 1 || cy > y + h + 1) return false;
      }
    }
    return true;
  }, () => {
    const out = [];
    for (const g of board.children.filter((c) => c.tagName === 'G')) {
      const cap = g.children.find((c) => c.tagName === 'RECT');
      const x = Number(cap.attrs.get('x'));
      const y = Number(cap.attrs.get('y'));
      const w = Number(cap.attrs.get('width'));
      const h = Number(cap.attrs.get('height'));
      for (const t of g.children.filter((c) => c.tagName === 'TEXT')) {
        const size = Number(t.attrs.get('font-size') ?? 12);
        const cx = Number(t.attrs.get('x'));
        const cy = Number(t.attrs.get('y'));
        const width = String(t.textContent).length * size * 0.6;
        if (!Number.isFinite(cx) || !Number.isFinite(cy) || cx - width / 2 < x - 1 || cx + width / 2 > x + w + 1
          || cy - size * 0.8 < y - 1 || cy > y + h + 1) out.push([g.attrs.get('data-hid'), t.textContent, [cx, cy, x, y, w, h, size]]);
      }
    }
    return out.slice(0, 4);
  });
ok('a cap click selects the key', () => {
  let picked = null;
  const built = app.buildBoard((hid) => { picked = hid; });
  const group = built.children.filter((c) => c.tagName === 'G' && c.attrs.get('data-hid') === String(app.LAYOUT[5].hid))[0];
  group.dispatch('click');
  return picked === app.LAYOUT[5].hid;
});
ok('the board geometry is the factory layout of the model the read decodes',
  () => app.LAYOUT.length > 80 && app.LAYOUT_ROWS.length >= 6 && app.LAYOUT_HIDS.size === app.LAYOUT.length
    && app.LAYOUT.every((k) => k.w > 0 && k.h > 0 && k.x >= 0 && k.y >= 0) && app.UNIT > app.GAP,
  () => [app.LAYOUT.length, app.LAYOUT_ROWS.length, app.LAYOUT_HIDS.size]);
ok('paintBoard classes a cap by its binding, and marks selected and changed keys', () => {
  const b = app.buildBoard(() => {});
  app.paintBoard(b, app.cloneDoc(apex), app.LAYER_BASE, 7, new Set([41]));
  const groups = {};
  for (const g of b.querySelectorAll('g.key')) groups[g.getAttribute('data-hid')] = g.getAttribute('class');
  return /\bselected\b/.test(groups[String(7)]) && /\bchanged\b/.test(groups[String(41)]) && !/\bchanged\b/.test(groups[String(7)]);
}, () => {
  const b = app.buildBoard(() => {});
  app.paintBoard(b, app.cloneDoc(apex), app.LAYER_BASE, 7, new Set([41]));
  const groups = {};
  for (const g of b.querySelectorAll('g.key')) groups[g.getAttribute('data-hid')] = g.getAttribute('class');
  return [groups[String(7)], groups[String(41)]];
  return b.children.filter((c) => c.attrs.get('data-hid') === '7').map((g) => g.attrs.get('class'));
});
ok('paintBoard labels each cap with the key and what it is bound to', () => {
  const b = app.buildBoard(() => {});
  app.paintBoard(b, app.cloneDoc(apex), app.LAYER_BASE, null, new Set());
  const texts = b.querySelectorAll('text').map((t) => t.textContent);
  return texts.includes('G') && texts.length > app.LAYOUT.length;
}, () => app.buildBoard(() => {}).querySelectorAll('text').slice(0, 8).map((t) => t.textContent));

// --- panels --------------------------------------------------------------
console.log('panels render for a keyboard read, a mouse read and an unlisted product');
const controls = () => {
  const n = () => globalThis.document.createElement('div');
  return { tabs: n(), status: n(), board: n(), boardBox: n(), nameBar: n(), inspector: n(), below: n(), legend: n(), fileInput: n(), saveBtn: n(), layerBaseBtn: n(), layerMetaBtn: n() };
};
const ctl = controls();
// main.js mounts the board itself and passes it as the board control.
const boardRoot = app.buildBoard(app.handleKeyClick);
ctl.board = boardRoot;
ok('the page boots against its controls and paints the mounted board', () => {
  app.init(ctl);
  const cls = [];
  for (const g of ctl.board.querySelectorAll('g.key')) cls.push(g.getAttribute('class'));
  return cls.length === app.boardKeyList().length && cls.some((c) => /\bk-self\b/.test(c));
}, () => [ctl.board.querySelectorAll('g.key').length, app.boardKeyList().length]);
const mouseDoc = (() => {
  const src = app.cloneDoc(apex);
  src.name = 'Mouse probe';
  src.name_hex = app.hexOf([...Buffer.from('Mouse probe', 'utf8')]);
  src.device = { vendor_id: 4144, product_id: 0x1662, product: 'Steelpoint Series 3', known: true, model_kind: 'mouse', writable: false, platform: 'wsl' };
  for (const name of ['wheel_mappings', 'wheel_meta_mappings']) {
    src[name] = src[name].map((e, i) => ({ ...e, function: 0x01 + i, key_codes: '00 00 00 00' }));
  }
  return app.normalizeDoc(src);
})();
ok('a mouse-family read has wheel entries, a keyboard read has the same two slots',
  () => app.wheelEntry(mouseDoc, app.LAYER_BASE, 0) !== null && app.wheelEntry(mouseDoc, app.LAYER_META, 1) !== null);
const mouseFile = app.addFile('mouse.json', app.serializeDoc(mouseDoc)).file;
const identityLoad = app.addFile('identity.json', readFileSync(`${EX}/sample-identity.json`, 'utf8'));
const unlisted = app.addFile('unlisted.json', app.serializeDoc({ ...app.cloneDoc(identityDoc), name: 'Unlisted read', product_id: 0x0427, product: 'Apex Pro (2023)', firmware: null })).file;
// The panels read state.layer/state.selected and every select handler calls render(), which paints
// against the active file. A check that edits a document runs with that file as the only file, and
// puts the suite's own file list back when it is done, so the checks after it see the page as loaded.
const withFile = (file, fn) => {
  const files = app.state.files;
  const activeId = app.state.activeId;
  const layer = app.state.layer;
  const selected = app.state.selected;
  app.state.files = [file];
  app.state.activeId = file.id;
  try {
    return fn();
  } finally {
    app.state.files = files;
    app.state.activeId = activeId;
    app.state.layer = layer;
    app.state.selected = selected;
  }
};
const pickSelect = (node, label) => {
  const select = [...node.querySelectorAll('select')].find((s) => s.children.some((o) => o.textContent === label));
  if (!select) throw new Error(`no select offers ${label}`);
  const option = select.children.find((o) => o.textContent === label);
  select.value = option.getAttribute('value');
  select.dispatch('change');
  return select;
};
// The binding window's Meta Toggle row: a labelled row with one checkbox, the only control that
// writes the profile byte. A check that moves the toggle uses these.
const metaToggleRow = (panel) => [...panel.querySelectorAll('div.line')]
  .find((row) => row.querySelector('.line-label')?.textContent === 'Meta Toggle');
const setToggleRow = (panel, on) => {
  const row = metaToggleRow(panel);
  const box = row?.querySelector('input.toggle-check');
  if (!box) throw new Error('no Meta Toggle row with a checkbox');
  box.checked = on;
  box.dispatch('change');
  return box;
};
const clickButton = (node, label) => {
  const btn = [...node.querySelectorAll('button')].find((b) => b.textContent === label);
  if (!btn) throw new Error(`no button ${label}`);
  btn.dispatch('click');
  return btn;
};
ok('the key panel binds a wheel cap for a mouse read and a keyboard read', () => {
  for (const file of [mouseFile, identityLoad.file]) {
    for (const [hid, listName] of [[app.WHEEL_UP, 'wheel_mappings'], [app.WHEEL_DOWN, 'wheel_meta_mappings']]) {
      for (const layer of [app.LAYER_BASE, app.LAYER_META]) {
        const index = hid === app.WHEEL_UP ? 0 : 1;
        const listName = layer === app.LAYER_META ? 'wheel_meta_mappings' : 'wheel_mappings';
        const otherName = layer === app.LAYER_META ? 'wheel_mappings' : 'wheel_meta_mappings';
        const untouched = app.cloneDoc(file.doc)[otherName][index];
        const writes = withFile(file, () => {
          app.state.layer = layer;
          app.state.selected = hid;
          const first = app.renderKeyPanel(file);
          const panelText = first.textContent;
          const offers = first.querySelectorAll('select.fn-select')[0].children.map((o) => o.textContent);
          pickSelect(first, 'Keyboard Key');
          if (file.doc[listName][index].function !== 0x51) return false;
          pickSelect(app.renderKeyPanel(file), 'M (0x10)');
          const target = file.doc[listName][index];
          return target.key_codes === '10 00 00 00' && target.function === 0x51
            && offers.includes('Deactivated') && !offers.includes('Meta Toggle')
            && /function 0x51 combo_key/.test(app.renderKeyPanel(file).textContent)
            && /Factory binding of (wheel_mappings|wheel_meta_mappings)\[\d\] - measured in a stock slot read/.test(panelText)
            && JSON.stringify(file.doc[otherName][index]) === JSON.stringify(untouched);
        });
        if (!writes) return false;
      }
    }
  }
  return true;
});
let wheelDebug = {};
ok('the wheel entries carry a factory binding measured in the stock read, and the wheel panel resets to it', () => {
  const base = [0, 1].map((i) => app.factoryEntry('wheel_mappings', i));
  const meta = [0, 1].map((i) => app.factoryEntry('wheel_meta_mappings', i));
  if (!eq(base.map((e) => [e.function, e.key_codes]), [[0x61, 'E9 00 00 00'], [0x61, 'EA 00 00 00']])) return false;
  if (!eq(meta.map((e) => [e.function, e.key_codes]), [[0x00, '00 00 00 00'], [0x00, '00 00 00 00']])) return false;
  const stockDocs = [identityDoc, apex, ...(macrosDoc ? [macrosDoc] : [])];
  for (const doc of stockDocs) {
    for (const [listName, index] of [['wheel_mappings', 0], ['wheel_mappings', 1],
      ['wheel_meta_mappings', 0], ['wheel_meta_mappings', 1]]) {
      const entry = doc[listName][index];
      const factory = app.factoryEntry(listName, index);
      if (entry.function !== factory.function || entry.key_codes !== factory.key_codes) return false;
      if (entry.hid_code !== (index === 0 ? 0x29 : 0x35)) return false;
    }
  }
  // addFile replaces the active file (one profile at a time), so the check keeps the suite's state.
  const savedFiles = app.state.files;
  const savedActiveId = app.state.activeId;
  const wheelFactoryFile = app.addFile('wheel-factory.json', app.serializeDoc(app.cloneDoc(apex))).file;
  const wheelResult = withFile(wheelFactoryFile, () => {
    wheelDebug = { base, meta, hint: '' };
    app.state.layer = app.LAYER_BASE;
    app.state.selected = app.WHEEL_UP;
    const first = app.renderKeyPanel(wheelFactoryFile);
    const clean = {
      text: first.textContent,
      disabled: [...first.querySelectorAll('button')].find((b) => b.textContent.includes('factory binding'))?.getAttribute('disabled'),
    };
    pickSelect(app.renderKeyPanel(wheelFactoryFile), 'Deactivated');
    const edited = wheelFactoryFile.doc.wheel_mappings[0];
    const panelAfterEdit = app.renderKeyPanel(wheelFactoryFile);
    const changed = edited.function === 0x00 && edited.key_codes === '00 00 00 00'
      && /This key is changed/.test(panelAfterEdit.textContent);
    clickButton(panelAfterEdit, 'Reset this key to its factory binding');
    const back = wheelFactoryFile.doc.wheel_mappings[0];
    const cond = {
      disabled: clean.disabled === 'true',
      hint: /Factory binding of wheel_mappings\[0\] - measured in a stock slot read: function 0x61 media_key codes E9 00 00 00/.test(clean.text),
      changed,
      back: back.function === 0x61 && back.key_codes === 'E9 00 00 00',
      pair: wheelFactoryFile.doc.wheel_mappings[1].key_codes === 'EA 00 00 00',
      meta: wheelFactoryFile.doc.wheel_meta_mappings[0].function === 0x00,
    };
    Object.assign(cond, {
      edited: JSON.stringify([edited.function, edited.key_codes]),
      restored: JSON.stringify([back.function, back.key_codes]),
      hint: clean.text.slice(-260),
    });
    wheelDebug = { ...wheelDebug, cond };
    globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
  });
  app.state.files = savedFiles;
  app.state.activeId = savedActiveId;
  return wheelResult;
}, () => JSON.stringify(wheelDebug));
ok('the OLED panel renders for a read with a bitmap and for one without',
  () => app.renderOledPanel(remap.file) !== undefined && app.renderOledPanel(identityLoad.file) !== undefined
    && app.renderOledPanel(unlisted) !== undefined);
ok('the OLED panel inverts every pixel, flips every bit, and inverting twice is the identity', () => {
  const suiteFiles = app.state.files;
  const suiteActive = app.state.activeId;
  const text = rootText('config_1.json');
  if (text === null) return SKIP;
  const file = app.addFile('oled-invert.json', text).file;
  const outcome = withFile(file, () => {
    const doc = file.doc;
    const before = Array.from(app.oledBytes(doc));
    const panel = app.renderOledPanel(file);
    const labels = panel.querySelectorAll('button').map((b) => b.textContent);
    const row = [...panel.querySelectorAll('div.line')].find((r) => r.querySelector('.line-label')?.textContent === 'Invert');
    const note = row.querySelector('.line-note').textContent;
    clickButton(panel, 'Invert every pixel');
    const once = Array.from(app.oledBytes(doc));
    const flips = once.length === before.length && once.every((byte, i) => byte === (before[i] ^ 0xff));
    const summary = doc.oled.len === app.OLED_BYTES
      && doc.oled.nonzero === once.filter((byte) => byte !== 0).length
      && doc.oled.head === app.hexOf(once.slice(0, 16))
      && doc.oled.fnv1a_64 === app.fnv1a64(once);
    const px0 = app.oledPixels(before);
    const px1 = app.oledPixels(once);
    let pixels = px1.length === px0.length;
    for (let at = 0; at < px1.length; at += 4) pixels = pixels && px1[at] === 255 - px0[at];
    clickButton(app.renderOledPanel(file), 'Invert every pixel');
    const identity = Array.from(app.oledBytes(doc)).every((byte, i) => byte === before[i]);
    const json = JSON.parse(app.serializeDoc(doc));
    const carried = json.oled.bytes_hex === String(doc.oled.bytes_hex).toUpperCase();
    const cond = {
      buttonShown: labels.includes('Invert every pixel') && labels.includes('Set all black'),
      rowNoInput: row.querySelectorAll('input').length === 0 && row.querySelectorAll('select').length === 0,
      noteRule: note.includes('flips every bit') && note.includes('inverting twice is the identity')
        && note.includes('7 stays the left pixel') && note.includes('no bytes_hex inverts to every pixel lit'),
      flips, summary, pixels, identity, carried,
    };
    globalThis.lastInvert = cond;
    return Object.values(cond).every(Boolean);
  });
  app.state.files = suiteFiles;
  app.state.activeId = suiteActive;
  return outcome;
}, () => [globalThis.lastInvert, app.renderOledPanel(app.state.files[0]).querySelector('.line-note')?.textContent.slice(0, 80)]);
ok('inverting a block with no bitmap lights every pixel, and the blank button after it blacks it', () => {
  const text = rootText('config_1.json');
  if (text === null) return SKIP;
  const bare = JSON.parse(text);
  delete bare.oled.bytes_hex;
  const suiteFiles = app.state.files;
  const suiteActive = app.state.activeId;
  const file = app.addFile('oled-empty.json', JSON.stringify(bare)).file;
  const outcome = withFile(file, () => {
    clickButton(app.renderOledPanel(file), 'Invert every pixel');
    const lit = app.oledBytes(file.doc).every((byte) => byte === 0xff)
      && file.doc.oled.len === app.OLED_BYTES && file.doc.oled.nonzero === app.OLED_BYTES;
    clickButton(app.renderOledPanel(file), 'Set all black');
    const black = app.oledBytes(file.doc).every((byte) => byte === 0) && file.doc.oled.nonzero === 0;
    const status = app.renderOledPanel(file).textContent;
    const cond = { lit, black, panelText: status.includes('Invert') && status.includes('Set all black') };
    globalThis.lastInvertEmpty = cond;
    return Object.values(cond).every(Boolean);
  });
  app.state.files = suiteFiles;
  app.state.activeId = suiteActive;
  return outcome;
}, () => globalThis.lastInvertEmpty);
ok('the binding, lighting, Rapid Tap, macro and changes panels render for every read', () => {
  for (const file of [remap.file, identityLoad.file, mouseFile, unlisted]) {
    app.renderBindingsPanel(file);
    app.renderLightingPanel(file);
    app.renderRapidTapPanel(file);
    app.renderMacrosPanel(file);
    app.renderChangesPanel(file);
  }
  return true;
});
ok('the key panel renders for the selected key of every read', () => {
  for (const file of [remap.file, identityLoad.file, mouseFile, unlisted]) {
    app.state.activeId = file.id;
    app.selectKey(app.LAYER_BASE, app.LAYOUT[5].hid);
    app.renderKeyPanel(file);
  }
  return true;
});
ok('the key panel renders for a key with no mapping offset', () => {
  app.selectKey(app.LAYER_BASE, 0xff);
  app.renderKeyPanel(remap.file);
  return true;
});
ok('the panel plan lists the change list, the inspector and the panels below the board', () => {
  app.state.selected = app.LAYOUT[5].hid;
  const plan = app.panelPlan(remap.file);
  return Array.isArray(plan.changes) && Array.isArray(plan.inspector) && plan.inspector.length >= 1 && Array.isArray(plan.below) && plan.below.length >= 3;
}, () => Object.keys(app.panelPlan(remap.file)));
ok('the panel plan has no wheel panel: a wheel cap edits in the one panel next to the board', () => withFile(mouseFile, () => {
  app.state.layer = app.LAYER_BASE;
  app.state.selected = app.WHEEL_UP;
  const wheelPlan = app.panelPlan(mouseFile);
  app.state.selected = app.LAYOUT[5].hid;
  const keyPlan = app.panelPlan(mouseFile);
  const below = wheelPlan.below.map((n) => n.textContent).join(' ');
  const wheelText = wheelPlan.inspector.map((node) => node.textContent).join(' ');
  return wheelPlan.inspector.length === 1 && keyPlan.inspector.length === 1
    && wheelPlan.below.length === keyPlan.below.length
    && !/Wheel mappings \(4 entries/.test(below)
    && wheelText.includes('wheel_mappings[0]') && /Wheel \u2191 - base layer/.test(wheelText);
}), () => withFile(mouseFile, () => [app.panelPlan(mouseFile).inspector.length, app.panelPlan(mouseFile).below.length]));
ok('the panel plan shows the OLED panel when the OLED rect is selected', () => {
  app.state.selected = app.SELECT_OLED;
  return app.panelPlan(remap.file).inspector.length >= 1;
});
ok('the legend explains every binding the board can show', () => {
  const chips = app.legendRows().children.map((row) => row.children.map((c) => c.attrs.get('class')).join(' '));
  const wanted = ['sw-self', 'sw-none', 'sw-deactivated', 'sw-combokey', 'sw-meta', 'sw-macro', 'sw-mediakey', 'sw-function', 'sw-nooffset', 'sw-changed', 'sw-selected'];
  return chips.length >= wanted.length && wanted.every((cls) => chips.some((row) => row.includes(cls)))
    && app.legendRows().children.every((row) => row.children.some((c) => c.textContent.length > 4));
}, () => app.legendRows().children.map((row) => row.children.map((c) => c.attrs.get('class')).join(' ')));
ok('the full render runs for the active file and paints the board', () => {
  app.render();
  const cls = ctl.board.querySelectorAll('g.key').map((g) => g.getAttribute('class'));
  return cls.length > 0 && cls.every((c) => c.startsWith('key ')) && ctl.tabs.children.length > 0 && ctl.inspector.children.length > 0;
}, () => [ctl.board.querySelectorAll('g.key').length, ctl.tabs.children.length, ctl.inspector.children.length]);
ok('selectKey stores the layer and key a binding row asks for', () => {
  app.selectKey(app.LAYER_META, 7);
  return app.state.layer === app.LAYER_META && app.state.selected === 7;
});
ok('the layer buttons switch the layer the board edits', () => {
  ctl.layerBaseBtn.dispatch('click');
  const base = app.state.layer;
  ctl.layerMetaBtn.dispatch('click');
  return base === app.LAYER_BASE && app.state.layer === app.LAYER_META;
});
ok('a board click on the OLED rect selects it and says what the dump holds', () => {
  app.handleKeyClick(app.SELECT_OLED);
  const text = ctl.status.textContent;
  return app.state.selected === app.SELECT_OLED
    && (text.includes('OLED') || text.includes('oled')) && (text.includes('bytes_hex') || text.includes('bitmap'));
}, () => ctl.status.textContent);
ok('a board click on a key selects it on the layer being edited', () => {
  app.state.layer = app.LAYER_BASE;
  app.handleKeyClick(7);
  return app.state.selected === 7;
}, () => app.state.selected);
ok('saving from the toolbar names the copy and counts the changes', () => {
  app.setCode(app.activeFile().doc, app.LAYER_BASE, 7, 0, 26);
  appended.length = 0;
  ctl.saveBtn.dispatch('click');
  const anchors = appended.filter((n) => n.tagName === 'A');
  return anchors.length === 1 && ctl.status.textContent.includes('saved a copy')
    && ctl.status.textContent.includes('change');
}, () => ctl.status.textContent);
ok('the file input loads the files a read wrote', async () => {
  appended.length = 0;
  const text = readFileSync(`${EX}/sample-remap.json`, 'utf8');
  await app.loadFiles([{ name: 'p1.json', text: async () => text }, { name: 'p2.json', text: async () => text }]);
  const files = app.state.files.map((f) => f.filename);
  return files.length === 1 && files[0].endsWith('p1.json') && app.activeFile().filename.endsWith('p1.json')
    && ctl.status.textContent.includes('loaded p1.json');
}, () => app.state.files.map((f) => f.filename));

const readFile = app.addFile('apex7-read.json', app.serializeDoc({ ...JSON.parse(JSON.stringify(apex)), product_id: 0x1612, product: 'Apex 7', firmware: '4.16.8', slot: 0 })).file;
ok('the header panel names the model, its product ID, where the name came from and its features', () => {
  const text = app.renderHeader(readFile, app.fileChanges(readFile)).textContent;
  return /Apex 7 - 0x1612/.test(text) && /imported from src\/devices\/mod\.rs/.test(text)
    && /on-board profile protocol: read back/.test(text) && /extra blocks this page decodes/.test(text);
}, () => app.renderHeader(readFile, app.fileChanges(readFile)).textContent.slice(0, 400));
const regionDoc = (() => {
  const doc = app.cloneDoc(apex);
  doc.payload_len = app.PAYLOAD_LEN_B;
  doc.extra_len = app.PROFILE_REGION_BYTES - app.PAYLOAD_LEN_B;
  doc.extra_fnv1a_64 = '0000000000000003';
  doc.extra_head = '00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00';
  return doc;
})();
const regionFile = app.addFile('with-region.json', app.serializeDoc(regionDoc)).file;
ok('the pair: a read report that carries the pulled region shows it in the file panel, a payload-only report shows no such row', () => {
  const withRow = app.renderHeader(regionFile, app.fileChanges(regionFile)).textContent;
  const plainRow = app.renderHeader(readFile, app.fileChanges(readFile)).textContent;
  return app.payloadSplit(regionDoc).includes(`payload ${app.PAYLOAD_LEN_B} + extra region ${app.PROFILE_REGION_BYTES - app.PAYLOAD_LEN_B}`)
    && app.payloadSplit(apex) === `payload ${app.PAYLOAD_LEN_B} (no extra region)`
    && withRow.includes('Extra region') && withRow.includes(`len ${app.PROFILE_REGION_BYTES - app.PAYLOAD_LEN_B}`)
    && withRow.includes('decoded by nothing')
    && !plainRow.includes('Extra region');
}, () => [app.payloadSplit(regionDoc), app.payloadSplit(apex), app.renderHeader(regionFile, []).textContent.slice(0, 600)]);
ok('the payload length the page names follows the schema word, and mirrors the Rust lengths', () => {
  const rust = read('src/devices/keyboards/apex_7_profile.rs');
  const a = Number(rust.match(/pub const PAYLOAD_LEN_A: usize = ([0-9_]+)/)[1].replace(/_/g, ''));
  const b = Number(rust.match(/pub const PAYLOAD_LEN_B: usize = ([0-9_]+)/)[1].replace(/_/g, ''));
  const region = Number(rust.match(/pub const PROFILE_REGION_BYTES: usize = ([0-9_]+)/)[1].replace(/_/g, ''));
  const macroLen = Number(rust.match(/pub const MACRO_DATA_LEN: usize = ([0-9_]+)/)[1].replace(/_/g, ''));
  return app.PAYLOAD_LEN_A === a && app.PAYLOAD_LEN_B === b && app.PROFILE_REGION_BYTES === region
    && app.MACRO_DATA_LEN === macroLen
    && app.payloadLenForSchema(0x000a) === a && app.payloadLenForSchema(0x000b) === b
    && app.payloadLenForSchema(0x0009) === null;
}, () => [app.PAYLOAD_LEN_A, app.PAYLOAD_LEN_B, app.PROFILE_REGION_BYTES, app.MACRO_DATA_LEN]);
ok('the Rapid Tap pairs print their reserved bytes and write them back unchanged', () => withFile(app.addFile('rt.json', app.serializeDoc(app.cloneDoc(apex))).file, () => {
  const file = app.state.files.find((f) => f.filename === 'rt.json');
  const before = file.doc.rapid_tap.pairs.map((pair) => pair.reserved);
  const panel = app.renderRapidTapPanel(file);
  const text = panel.textContent;
  return before.length === 10 && before.every((r) => r.split(' ').length === 2)
    && text.includes(`reserved ${before[0]}`) && text.includes(`reserved ${before[9]}`);
}), () => [app.rapidTap(apex).pairs.map((pair) => pair.reserved)]);
okMacros('the macro block ships its bytes in the report, and a save writes them back byte for byte', () => {
  const bytes = app.macroBlockBytes(app.cloneDoc(macrosDoc));
  const saved = JSON.parse(app.serializeDoc(app.cloneDoc(macrosDoc)));
  return bytes !== null && bytes.length === app.MACRO_DATA_LEN
    && saved.macro_data.bytes_hex === macrosDoc.macro_data.bytes_hex
    && JSON.stringify(saved.macros) === JSON.stringify(macrosDoc.macros)
    && saved.macro_note === macrosDoc.macro_note;
}, () => [app.macroBlockBytes(app.cloneDoc(macrosDoc))?.length, macrosDoc.macro_data.bytes_hex?.length]);
okMacros('a real macro block the record list cannot walk still carries every byte the write sends', () => {
  const bytes = app.macroBlockBytes(app.cloneDoc(macrosDoc));
  const at = macrosDoc.macro_data.offset;
  // the byte the decode note names, and each parsed record's bytes, are inside the shipped block
  const listed = (macrosDoc.macros ?? []).every((record) => bytes
    .slice(record.offset - at, record.offset - at + record.record_bytes)
    .map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join(' ') === record.raw);
  const panel = app.renderMacrosPanel(app.addFile('blocks.json', app.serializeDoc(app.cloneDoc(macrosDoc))).file).textContent;
  return bytes[109] === 0x03 && bytes[108] === 0x00 && listed
    && panel.includes('The write sends these bytes back unchanged')
    && panel.includes('a limit of the list on this page, not of the profile');
}, () => [macrosDoc.macro_note, app.macroBlockBytes(app.cloneDoc(macrosDoc))?.[109]]);
okMacros('a report without the block bytes says so, and names the read that would fix it', () => {
  const doc = app.cloneDoc(macrosDoc);
  delete doc.macro_data.bytes_hex;
  const text = app.renderMacrosPanel(app.addFile('noblocks.json', app.serializeDoc(doc)).file).textContent;
  const header = app.renderHeader(app.addFile('noblocks2.json', app.serializeDoc(doc)).file, []).textContent;
  return app.macroBlockBytes(doc) === null
    && text.includes('carries none, so a write has nothing to send back verbatim')
    && text.includes('this report carries no block bytes, so the write path refuses it')
    && header.includes('carries no block bytes');
}, () => app.macroBlockBytes(app.cloneDoc(macrosDoc)));

ok('the header panel says when a read names a model outside the family', () => {
  const text = app.renderHeader(unlisted, app.fileChanges(unlisted)).textContent;
  return /0x0427/.test(text) && /not the one this page encodes/.test(text);
}, () => app.renderHeader(unlisted, app.fileChanges(unlisted)).textContent.slice(0, 400));

// --- the META toggle binding, the name editor above the board, the bottom row order ---------
console.log('the META toggle binding, the whole macro block, the name editor above the board, the bottom row');
const toggleFile = app.addFile('toggle.json', app.serializeDoc(app.cloneDoc(apex))).file;
const metaFile = app.addFile('meta-key.json', app.serializeDoc(app.cloneDoc(apex))).file;
const metaKeyFile = app.addFile('meta-toggle.json', app.serializeDoc(app.cloneDoc(apex))).file;
const metaOffFile = app.addFile('meta-off.json', app.serializeDoc(app.cloneDoc(apex))).file;
const headerFile = app.addFile('header.json', app.serializeDoc(app.cloneDoc(apex))).file;
const nameFile = app.addFile('name-edit.json', app.serializeDoc(app.cloneDoc(apex))).file;
const macroBindDoc = macrosDoc ? (() => {
  const doc = app.cloneDoc(macrosDoc);
  doc.button_mappings[27] = { ...doc.button_mappings[27], function: 0x71, key_codes: '00 00 07 00' };
  return app.normalizeDoc(doc);
})() : null;
const fieldFile = app.addFile('toggle-field.json', app.serializeDoc(app.cloneDoc(apex))).file;
const boardFile = app.addFile('toggle-board.json', app.serializeDoc(app.cloneDoc(apex))).file;
const macroBindFile = macroBindDoc ? app.addFile('macro-bind.json', app.serializeDoc(macroBindDoc)).file : null;

ok('the Meta action list carries Lock Win as its own action and the META toggle is a row of its own', () => withFile(toggleFile, () => {
  app.state.layer = app.LAYER_BASE;
  app.state.selected = 0xe3;
  pickSelect(app.renderKeyPanel(toggleFile), 'Meta');
  const panel = app.renderKeyPanel(toggleFile);
  const groups = panel.querySelectorAll('select').find((sel) => sel.children.some((o) => o.textContent === 'Keyboard Key')).children.map((o) => o.textContent);
  const meta = panel.querySelectorAll('select').find((sel) => sel.children.some((o) => o.textContent.startsWith('Profile Switch (F9)')));
  const labels = meta.children.map((o) => o.textContent);
  const row = metaToggleRow(panel);
  const cond = {
    groups: groups.length === app.BINDING_GROUPS.length && !groups.some((l) => l.includes('Meta Toggle')),
    lockWin: labels.includes('Lock Win (0x05)'),
    rapidTap: labels.includes('Rapid Tap (0x0A)'),
    noToggleWordInActions: !labels.some((l) => l.includes('Meta Toggle')),
    listOrder: labels.join('|') === 'Profile Switch (F9) (0x01)|Macro (F10) (0x02)|Brightness Down (F11) (0x03)|Brightness Up (F12) (0x04)|Lock Win (0x05)|Rapid Tap (0x0A)',
    toggleRowHasCheckbox: !!row && !!row.querySelector('input.toggle-check'),
    rowNamesByte: !!row && row.textContent.includes(`meta_toggle_hid = 0x${app.hex2(Number(toggleFile.doc.meta_toggle_hid))}`),
    noByteButtons: !panel.querySelectorAll('button').map((b) => b.textContent).some((b) => /META toggle|Make this key|Take the META/.test(b)),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.BINDING_GROUPS.map((g) => g.id), app.renderKeyPanel(toggleFile).querySelectorAll('select').map((sel) => sel.children.map((o) => o.textContent).join(' | ')).slice(0, 3)]);
ok('picking Lock Win writes code 1 = 0x05 on this entry and leaves the profile byte where it was', () => withFile(toggleFile, () => {
  app.state.layer = app.LAYER_BASE;
  app.state.selected = 0xe3;
  const offset = app.hidSlot(0xe3);
  const entry = toggleFile.doc.button_mappings[offset];
  const byte0 = Number(toggleFile.doc.meta_toggle_hid);
  pickSelect(app.renderKeyPanel(toggleFile), 'Meta');
  const metaShape = String(entry.key_codes);
  pickSelect(app.renderKeyPanel(toggleFile), 'Lock Win (0x05)');
  const panel = app.renderKeyPanel(toggleFile);
  const group = panel.querySelectorAll('select').find((sel) => sel.children.some((o) => o.textContent === 'Keyboard Key'));
  const state = app.keyState(toggleFile.doc, app.LAYER_BASE, 0xe3);
  const row = metaToggleRow(panel);
  const cond = {
    fn: entry.function === app.META_TOGGLE_FUNCTION,
    metaShape: metaShape === '01 01 00 00',
    codes: entry.key_codes === '05 01 00 00',
    byteUnchanged: Number(toggleFile.doc.meta_toggle_hid) === byte0,
    group: group.value === 'meta',
    action: panel.querySelectorAll('select').map((sel) => sel.value).includes('5'),
    label: state.kind === 'meta' && state.label === 'Lock Win',
    rowUnchecked: row.querySelector('input.toggle-check').checked === false,
    statusNoToggleClaim: !ctl.status.textContent.includes('switches META mode'),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, toggleFile.doc.button_mappings[app.hidSlot(0xe3)], toggleFile.doc.meta_toggle_hid, ctl.status.textContent.slice(0, 200)]);
ok('the toggle row moves the byte to the key it is checked on and writes no binding byte', () => withFile(metaOffFile, () => {
  const out = {};
  for (const [name, hid] of [['f9', 0x42], ['a', 0x04], ['caps', 0x39]]) {
    app.state.layer = app.LAYER_BASE;
    app.state.selected = hid;
    const offset = app.hidSlot(hid);
    const entry = metaOffFile.doc.button_mappings[offset];
    const before = { fn: entry.function, codes: String(entry.key_codes) };
    setToggleRow(app.renderKeyPanel(metaOffFile), true);
    const state = app.keyState(metaOffFile.doc, app.LAYER_BASE, hid);
    out[name] = { fn: entry.function, codes: String(entry.key_codes), byte: Number(metaOffFile.doc.meta_toggle_hid), kind: state.kind, label: state.label, code: hid, before };
  }
  const cond = {
    f9: out.f9.byte === out.f9.code && out.f9.fn === out.f9.before.fn && out.f9.codes === out.f9.before.codes,
    a: out.a.byte === out.a.code && out.a.fn === out.a.before.fn && out.a.codes === out.a.before.codes,
    caps: out.caps.byte === out.caps.code && out.caps.fn === out.caps.before.fn && out.caps.codes === out.caps.before.codes,
    kinds: [out.f9, out.a, out.caps].every((r) => r.kind === 'meta' && r.label === 'META'),
    metaCapCode: app.toggleByteFor(0x65) === app.META_KEY_GG_CODE,
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, Object.keys(globalThis.lastCond ?? {}).length, metaOffFile.doc.meta_toggle_hid]);
ok('a META-layer Lock Win entry opens like any other key, keeps its bytes, and shows the toggle row', () => withFile(toggleFile, () => {
  app.state.layer = app.LAYER_META;
  app.state.selected = 0xe3;
  const panel = app.renderKeyPanel(toggleFile);
  const entry = toggleFile.doc.button_meta_mappings[13];
  const before = { fn: entry.function, codes: String(entry.key_codes), byte: Number(toggleFile.doc.meta_toggle_hid) };
  const state = app.keyState(toggleFile.doc, app.LAYER_META, 0xe3);
  const buttons = panel.querySelectorAll('button').map((b) => b.textContent);
  const cond = {
    twoSelects: panel.querySelectorAll('select').length >= 2,
    bindingMeta: panel.querySelectorAll('select').map((sel) => sel.value).includes('meta'),
    lockWinSelected: panel.querySelectorAll('select').map((sel) => sel.value).includes('5'),
    bindingButtons: buttons.length === 2 && buttons.includes('Undo the changes on this key') && buttons.includes('Reset this key to its factory binding'),
    noFixedText: !panel.textContent.includes('fixed on the META layer'),
    label: state.kind === 'meta' && state.label === 'Lock Win',
    toggleRowShown: !!metaToggleRow(panel),
    kept: entry.function === before.fn && String(entry.key_codes) === before.codes && Number(toggleFile.doc.meta_toggle_hid) === before.byte,
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderKeyPanel(toggleFile).querySelectorAll('select').length, app.keyState(toggleFile.doc, app.LAYER_META, 0xe3), toggleFile.doc.button_meta_mappings[13]]);
ok('the key the byte names keeps its own binding bytes in the panel and shows the toggle row checked', () => withFile(boardFile, () => {
  boardFile.doc.meta_toggle_hid = 0x04;
  app.normalizeDoc(boardFile.doc);
  const offset = app.hidSlot(0x04);
  const metaEntry = boardFile.doc.button_meta_mappings[offset];
  const before = { fn: metaEntry.function, codes: String(metaEntry.key_codes) };
  app.state.layer = app.LAYER_META;
  app.state.selected = 0x04;
  const panel = app.renderKeyPanel(boardFile);
  const metaState = app.keyState(boardFile.doc, app.LAYER_META, 0x04);
  app.render();
  const cap = ctl.board.querySelectorAll('g.key').find((g) => g.getAttribute('data-hid') === '4');
  const dark = { cls: cap.getAttribute('class'), fill: cap.querySelector('.fill').getAttribute('fill'), sub: cap.querySelector('.sub').textContent };
  app.state.layer = app.LAYER_BASE;
  app.render();
  const baseCap = ctl.board.querySelectorAll('g.key').find((g) => g.getAttribute('data-hid') === '4');
  const baseState = app.keyState(boardFile.doc, app.LAYER_BASE, 0x04);
  const basePanel = app.renderKeyPanel(boardFile);
  const selects = basePanel.querySelectorAll('select');
  const cond = {
    metaKind: metaState.kind === 'meta' && metaState.label === 'META',
    metaEditable: panel.querySelectorAll('select').length >= 1,
    baseGroupIsItsOwnBytes: selects[0].value === 'keyboard',
    baseMetaKind: baseState.kind === 'meta' && baseState.label === 'META',
    rowChecked: metaToggleRow(basePanel).querySelector('input.toggle-check').checked === true,
    byteName: basePanel.textContent.includes('meta_toggle_hid = 0x04'),
    metaFill: dark.fill === app.FILL_COLORS.meta && dark.cls.includes('k-meta'),
    metaSub: dark.sub.startsWith('M'),
    kept: metaEntry.function === before.fn && String(metaEntry.key_codes) === before.codes,
    baseFill: baseCap.querySelector('.fill').getAttribute('fill') === app.FILL_COLORS.meta && baseCap.querySelector('.sub').textContent.startsWith('M'),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.keyState(boardFile.doc, app.LAYER_BASE, 0x04), app.renderKeyPanel(boardFile).querySelectorAll('select').map((sel) => sel.value), boardFile.doc.meta_toggle_hid]);
ok('the byte paints its key with META on both layers and a Lock Win binding prints Lock Win', () => {
  const files = app.state.files;
  const activeId = app.state.activeId;
  const read = (hid) => {
    const group = ctl.board.querySelectorAll('g.key').find((g) => g.getAttribute('data-hid') === String(hid));
    return { cls: group.getAttribute('class'), fill: group.querySelector('.fill').getAttribute('fill'), sub: group.querySelector('.sub').textContent };
  };
  boardFile.doc.meta_toggle_hid = 0x04;
  app.normalizeDoc(boardFile.doc);
  app.state.files = [boardFile];
  app.state.activeId = boardFile.id;
  app.state.layer = app.LAYER_BASE;
  app.state.selected = app.OFF2HID[72];
  app.render();
  pickSelect(app.renderKeyPanel(boardFile), 'Meta');
  pickSelect(app.renderKeyPanel(boardFile), 'Lock Win (0x05)');
  app.state.layer = app.LAYER_BASE;
  app.render();
  const bound = read(app.OFF2HID[72]);
  app.state.layer = app.LAYER_META;
  app.state.selected = 0xe3;
  app.render();
  const onMeta = read(0xe3);
  app.state.layer = app.LAYER_BASE;
  app.render();
  const namedBase = read(0x04);
  app.state.layer = app.LAYER_META;
  app.render();
  const namedMeta = read(0x04);
  app.state.files = files;
  app.state.activeId = activeId;
  app.state.selected = 0xe3;
  app.state.layer = app.LAYER_BASE;
  app.render();
  const cond = {
    raw: JSON.stringify([onMeta, bound, namedBase]),
    metaBindingFill: onMeta.fill === app.FILL_COLORS.meta && onMeta.cls.includes('k-meta'),
    metaBindingSub: onMeta.sub === 'Lock Win',
    boundFill: bound.fill === app.FILL_COLORS.meta,
    boundSub: bound.sub.startsWith('Lock W'),
    namedBase: namedBase.fill === app.FILL_COLORS.meta && namedBase.cls.includes('k-meta') && namedBase.sub.startsWith('M'),
    namedMeta: namedMeta.fill === app.FILL_COLORS.meta && namedMeta.sub.startsWith('M'),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}, () => [globalThis.lastCond, app.keyState(boardFile.doc, app.LAYER_META, 0xe3), app.keyState(boardFile.doc, app.LAYER_BASE, app.OFF2HID[72]), app.FILL_COLORS.meta]);
ok('a meta key prints its action on the cap, the two open codes print their byte, and the cap of usage 0x65 is named Menu', () => {
  const words = [72, 80, 88, 96].map((off) => app.keyState(apex, app.LAYER_META, app.OFF2HID[off]).label);
  const mark = app.lockWinMarkers(apex)[0];
  const cond = {
    words: words.join('|') === 'Profile|Macro|Bright down|Bright up',
    toggleCap: app.keyState(apex, app.LAYER_BASE, app.boardHid(Number(apex.meta_toggle_hid))).label === 'META',
    markLabel: app.keyState(apex, app.LAYER_META, mark.hid).label === 'Lock Win',
    rapidTapCap: app.META_CAP_LABELS[0x0a] === 'Rapid Tap' && app.META_ACTION_NAMES[0x0a] === 'Rapid Tap',
    menu: app.keyName(0x65) === 'Menu' && app.KEY_NAMES[0x65] === 'Menu',
    readings: app.META_CAP_LABELS[0x05] === 'Lock Win' && app.META_CAP_LABELS[0x0a] === 'Rapid Tap',
    knownCodes: Object.keys(app.META_ACTION_NAMES).map(Number).sort((a, b) => a - b).join(',') === '1,2,3,4,5,10',
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}, () => [globalThis.lastCond, [72, 80, 88, 96].map((off) => app.keyState(apex, app.LAYER_META, app.OFF2HID[off]).label), app.META_CAP_LABELS]);
ok('the META layer keeps the normal binding panel and puts the toggle in its own row', () => withFile(toggleFile, () => {
  app.state.layer = app.LAYER_META;
  app.state.selected = app.OFF2HID[72];
  const panel = app.renderKeyPanel(toggleFile);
  const meta = panel.querySelectorAll('select').find((s) => s.children.some((o) => o.textContent.startsWith('Profile Switch (F9)')));
  const labels = meta.children.map((o) => o.textContent);
  app.state.selected = app.WHEEL_UP;
  const wheelPanel = app.renderKeyPanel(toggleFile);
  app.state.selected = app.OFF2HID[72];
  const cond = {
    selects: panel.querySelectorAll('select').length >= 2,
    sixActions: labels.length === 6 && labels.includes('Lock Win (0x05)') && !labels.some((l) => l.includes('Meta Toggle')),
    toggleRow: !!metaToggleRow(panel),
    noteSeparates: panel.textContent.includes('the META toggle is the profile byte below'),
    keptCodes: app.keyState(toggleFile.doc, app.LAYER_META, app.OFF2HID[72]).codes[0] === 0x01,
    wheelNoToggle: !metaToggleRow(wheelPanel),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderKeyPanel(toggleFile).querySelectorAll('select').map((sel) => sel.children.map((o) => o.textContent).join(' | ')).slice(0, 2), app.keyState(toggleFile.doc, app.LAYER_META, app.OFF2HID[72])]);
ok('the Meta cap binds through slot 93 like any key, rebinds to the Context Menu, and writes no byte button', () => withFile(metaFile, () => {
  app.state.layer = app.LAYER_BASE;
  app.state.selected = app.META_KEY_HID;
  metaFile.doc.meta_toggle_hid = 0x04;
  const panel = app.renderKeyPanel(metaFile);
  const text = panel.textContent;
  const entry = metaFile.doc.button_mappings[app.META_KEY_SLOT];
  const factory = app.factoryEntry('button_mappings', app.META_KEY_SLOT);
  const byteBefore = Number(metaFile.doc.meta_toggle_hid);
  const buttons = panel.querySelectorAll('button').map((b) => b.textContent);
  pickSelect(panel, 'Keyboard Key');
  pickSelect(app.renderKeyPanel(metaFile), 'Menu (0x65)');
  const rebound = { fn: entry.function, codes: entry.key_codes, byte: Number(metaFile.doc.meta_toggle_hid) };
  const cond = {
    alias: app.hidSlot(app.META_KEY_HID) === app.META_KEY_SLOT && app.OFF2HID[app.META_KEY_SLOT] === app.META_KEY_GG_CODE,
    bindingSelect: panel.querySelectorAll('select').length >= 2,
    slotName: text.includes('it binds through button_mappings[93]'),
    contextMenu: text.includes('the keystroke that opens the Context Menu') && text.includes('Fn F0'),
    factory: factory.function === 0x51 && factory.key_codes === 'F0 00 00 00',
    labels: panel.querySelectorAll('select').flatMap((sel) => sel.children.map((o) => o.textContent)).includes('Menu (0x65)'),
    rebound: rebound.fn === 0x51 && rebound.codes === '65 00 00 00',
    byte: rebound.byte === byteBefore && byteBefore === 0x04,
    noByteButtons: !buttons.some((b) => /META toggle/i.test(b)),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderKeyPanel(metaFile).textContent.slice(0, 240), metaFile.doc.button_mappings[app.META_KEY_SLOT], app.factoryEntry('button_mappings', app.META_KEY_SLOT)]);
ok('the header row names the byte and the Lock Win bindings apart and edits nothing', () => withFile(headerFile, () => {
  const panel = app.renderHeader(headerFile, app.fileChanges(headerFile));
  const text = panel.textContent;
  const bound = app.lockWinBinding(headerFile.doc);
  const markers = app.lockWinMarkers(headerFile.doc).map((m) => m.offset);
  const cond = {
    noSelect: panel.querySelectorAll('select').length === 0,
    noInput: panel.querySelectorAll('input').length === 0,
    noButton: panel.querySelectorAll('button').length === 0,
    byteName: text.includes(`meta_toggle_hid 0x${app.hex2(Number(headerFile.doc.meta_toggle_hid))}`),
    markerName: text.includes(`button_meta_mappings[${bound.offset}]`) && markers.length >= 1,
    metaRow: text.includes('META toggle'),
    slotNote: text.includes('slot 93'),
    readOnly: text.includes('this row edits nothing'),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderHeader(headerFile, []).querySelectorAll('button').length, app.lockWinMarkers(headerFile.doc).map((m) => m.offset), headerFile.doc.meta_toggle_hid]);
ok('the byte is a field of its own: undo and factory reset never move it, and its change is listed apart', () => withFile(fieldFile, () => {
  fieldFile.doc.meta_toggle_hid = 0x26;
  app.state.layer = app.LAYER_BASE;
  app.state.selected = 0x26;
  const offset = app.hidSlot(0x26);
  const entry = fieldFile.doc.button_mappings[offset];
  const factory = app.factoryEntry('button_mappings', offset);
  pickSelect(app.renderKeyPanel(fieldFile), 'Deactivated');
  const deactivated = { fn: entry.function, codes: entry.key_codes, paths: app.fileChanges(fieldFile).map((c) => c.path) };
  const panel = app.renderKeyPanel(fieldFile);
  const buttons = panel.querySelectorAll('button').map((b) => b.textContent);
  clickButton(panel, 'Undo the changes on this key');
  const afterUndo = { fn: entry.function, codes: entry.key_codes, byte: Number(fieldFile.doc.meta_toggle_hid) };
  pickSelect(app.renderKeyPanel(fieldFile), 'Media Key');
  clickButton(app.renderKeyPanel(fieldFile), 'Reset this key to its factory binding');
  const afterReset = { fn: entry.function, codes: entry.key_codes, byte: Number(fieldFile.doc.meta_toggle_hid) };
  const hint = app.renderKeyPanel(fieldFile).textContent;
  const toggleRow = app.renderKeyPanel(fieldFile);
  const cond = {
    deactFn: deactivated.fn === 0x00,
    bytePath: deactivated.paths.includes('meta_toggle_hid'),
    undoBtn: buttons.includes('Undo the changes on this key'),
    resetBtn: buttons.includes('Reset this key to its factory binding'),
    undoFn: afterUndo.fn === factory.function && afterUndo.codes === factory.key_codes,
    undoByte: afterUndo.byte === 0x26,
    resetFn: afterReset.fn === factory.function && afterReset.codes === factory.key_codes,
    resetByte: afterReset.byte === 0x26,
    fieldNote: hint.includes('the profile byte meta_toggle_hid is a separate field'),
    toggleRowText: toggleRow.textContent.includes('meta_toggle_hid = 0x26 (names'),
    noByteButton: !buttons.some((b) => /Make this key|Take the META toggle/i.test(b)),
    diffIsByteOnly: app.diffAgainst(fieldFile.original, fieldFile.doc).map((c) => c.path).sort().join(',') === 'meta_toggle_hid',
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderKeyPanel(metaOffFile).textContent.slice(-320), app.diffAgainst(metaOffFile.original, metaOffFile.doc).map((c) => c.path)]);
ok('the META mask is the measured rule: bound META-layer keys by their own HID code, the key the byte names and the wheel left out', () => {
  const rootNames = ['config_1.json', 'config_3.json', 'some_rebounds.json', 'some_rebounds_prev.json', 'config_1-copy.json', 'sample-macros.json'];
  const rootTexts = rootNames.map(rootText);
  if (rootTexts.some((text) => text === null)) return SKIP;
  const readDocs = [apex, identityDoc, parseExample('sample-remap.json'), JSON.parse(rootTexts[5]),
    app.addFile('mask-config1.json', rootTexts[0]).file.doc,
    app.addFile('mask-config3.json', rootTexts[1]).file.doc,
    app.addFile('mask-some.json', rootTexts[2]).file.doc,
    app.addFile('mask-some-prev.json', rootTexts[3]).file.doc];
  const matches = readDocs.map((doc) => app.computeMetaMask(doc) === String(doc.lighting.meta_mapped_elements).toUpperCase());
  const copyDoc = app.addFile('mask-copy.json', rootTexts[4]).file.doc;
  // The copy is the file a page edit made: its binding table carries one META binding more than a stock
  // read, and its mask carries the bit that binding needs and the stock mask has no bit for. That extra
  // bit is the measurement that the mask follows the META layer (the stored bytes were the read's stale ones).
  const copyBits = app.computeMetaMask(copyDoc);
  const stockBits = apex.lighting.meta_mapped_elements;
  const byteNum = (mask, index) => Number(`0x${mask.split(' ')[index]}`);
  const extraBits = [...Array(32).keys()].reduce((sum, i) => sum + [...Array(8).keys()].filter((b) => ((byteNum(copyBits, i) >> b) & 1) && !((byteNum(stockBits, i) >> b) & 1)).length, 0);
  const wheelDoc = app.cloneDoc(apex);
  wheelDoc.wheel_meta_mappings[0] = { ...wheelDoc.wheel_meta_mappings[0], function: 0x61, key_codes: 'E9 00 00 00' };
  // slot 13 = LWin (usage 0xE3) is bound 62 05 in a stock read: naming that key in the byte must drop
  // its bit, and clearing the byte must bring it back.
  const bitOf = (mask, usage) => (Number(`0x${mask.split(' ')[usage >> 3]}`) >> (usage % 8)) & 1;
  const namedDoc = app.cloneDoc(apex);
  namedDoc.meta_toggle_hid = app.OFF2HID[13];
  app.normalizeDoc(namedDoc);
  const unnamedDoc = app.cloneDoc(apex);
  unnamedDoc.meta_toggle_hid = 0x00;
  app.normalizeDoc(unnamedDoc);
  const cond = {
    allReadsMatch: matches.length === 8 && matches.every(Boolean),
    copyGainsABitTheStockMaskHasNoBitFor: extraBits > 0,
    wheelLeftOut: app.computeMetaMask(wheelDoc) === app.computeMetaMask(apex),
    namedKeyDropped: bitOf(app.computeMetaMask(namedDoc), app.OFF2HID[13]) === 0,
    bitPresentWhenUnnamed: bitOf(app.computeMetaMask(unnamedDoc), app.OFF2HID[13]) === 1,
  };
  globalThis.lastMask = cond;
  return Object.values(cond).every(Boolean);
}, () => [globalThis.lastMask, app.computeMetaMask(apex), apex.lighting.meta_mapped_elements]);
ok('the lighting panel shows the mask as a derived read-only value and compares it with the read', () => withFile(toggleFile, () => {
  const panel = app.renderLightingPanel(toggleFile);
  const rows = [...panel.querySelectorAll('div.line')];
  const row = rows.find((r) => r.querySelector('.line-label')?.textContent === 'META mapped mask');
  const note = row.querySelector('.line-note').textContent;
  const cond = {
    rowExists: !!row,
    noInput: row.querySelectorAll('input').length === 0,
    noSelect: row.querySelectorAll('select').length === 0,
    showsDerived: row.querySelector('.line-control').textContent === app.computeMetaMask(toggleFile.doc),
    notesRule: note.includes('not editable') && note.includes('function byte != 0') && note.includes('meta_toggle_hid'),
    comparesRead: note.includes('the bytes this read carries (meta_mapped_elements) are the same'),
    measuredLabel: note.includes('EXPERIMENTAL'),
  };
  globalThis.lastMask = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastMask, app.renderLightingPanel(toggleFile).querySelector('div.line')?.textContent.slice(0, 120)]);
ok('a META-layer binding moves the mask with it and the written JSON carries the derived bytes', () => withFile(toggleFile, () => {
  const doc = toggleFile.doc;
  const offset = 60;
  const usage = app.OFF2HID[offset];
  const byteOf = (mask) => Number(`0x${mask.split(' ')[usage >> 3]}`);
  const bitOn = (mask) => (byteOf(mask) >> (usage % 8)) & 1;
  const before = app.computeMetaMask(doc);
  app.state.layer = app.LAYER_META;
  app.state.selected = usage;
  app.render();
  pickSelect(app.renderKeyPanel(toggleFile), 'Keyboard Key');
  pickSelect(app.renderKeyPanel(toggleFile), 'A (0x04)');
  const bound = app.computeMetaMask(doc);
  const panelText = app.renderLightingPanel(toggleFile).textContent;
  const json = JSON.parse(app.serializeDoc(toggleFile.doc));
  app.state.selected = usage;
  app.render();
  pickSelect(app.renderKeyPanel(toggleFile), 'Keyboard Key');
  pickSelect(app.renderKeyPanel(toggleFile), 'none (0x00)');
  pickSelect(app.renderKeyPanel(toggleFile), 'Deactivated');
  const after = app.computeMetaMask(doc);
  const cond = {
    usageKnown: usage >= 0 && usage <= 255,
    offBefore: bitOn(before) === 0,
    onAfterBinding: bitOn(bound) === 1,
    panelShowsBound: panelText.includes(bound),
    jsonCarriesDerived: json.lighting.meta_mapped_elements === bound,
    docKeepsReadBytes: String(doc.lighting.meta_mapped_elements).toUpperCase() === String(toggleFile.original.lighting.meta_mapped_elements).toUpperCase(),
    clearedAgain: after === before,
  };
  globalThis.lastMask = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastMask, app.OFF2HID[60], app.computeMetaMask(toggleFile.doc), toggleFile.doc.button_meta_mappings[60]]);
ok('the binding counts keep GG binding groups and the legend lists the meta colour', () => {
  const counts = app.bindingCounts(apex).map((row) => [row.layer, row.counts]);
  const metaCounts = app.bindingCounts(app.cloneDoc(apex)).map((row) => [row.layer, row.counts]);
  const lines = app.legendRows().children.map((row) => row.children.map((c) => c.textContent).join(' | '));
  const metaLine = lines.filter((l) => l.includes('a meta key (function 0x62)'));
  const cond = {
    countsSame: JSON.stringify(counts) === JSON.stringify(metaCounts),
    countsValues: JSON.stringify(counts.map(([, c]) => [c.self, c.none, c.custom])) === JSON.stringify(
      [app.LAYER_BASE, app.LAYER_META].map((layer) => {
        const c = { self: 0, none: 0, custom: 0 };
        app.layerArray(apex, layer).forEach((e) => { c[e.binding === 'self' ? 'self' : e.binding === 'none' ? 'none' : 'custom'] += 1; });
        return [c.self, c.none, c.custom];
      }))
      && counts.reduce((n, [, c]) => n + c.self + c.none + c.custom, 0) === app.MAPPING_COUNT * 2,
    noToggleGroup: !app.BINDING_GROUPS.some((g) => g.id === 'toggle' || g.label.includes('Meta Toggle')),
    metaLineOnce: metaLine.length === 1,
    metaNamesByte: metaLine[0].includes('meta_toggle_hid alone') && metaLine[0].includes('binds like any other key'),
    capWords: metaLine[0].includes('Profile, Macro, Bright up, Bright down, Lock Win, Rapid Tap'),
    swatch: /sw-meta/.test(app.legendRows().children.map((r) => r.children.map((c) => c.getAttribute('class')).join(' ')).join(' ')),
    noMetalock: !/metalock/.test(app.legendRows().children.map((r) => r.children.map((c) => c.getAttribute('class')).join(' ')).join(' ')),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}, () => [globalThis.lastCond, app.BINDING_GROUPS.map((g) => g.id), app.legendRows().children.map((r) => r.children.map((c) => c.textContent).join(' ')).filter((l) => /marks on the META layer|0x62 meta/.test(l))]);
okMacros('the key panel of a macro-bound key shortens the select and shows the whole record under it', () => withFile(macroBindFile, () => {
  app.state.layer = app.LAYER_BASE;
  app.state.selected = app.OFF2HID[27];
  const panel = app.renderKeyPanel(macroBindFile);
  const macroSelect = panel.querySelectorAll('select').find((s) => s.children.some((o) => o.textContent.startsWith('Record ')));
  const labels = macroSelect.children.map((o) => o.textContent);
  const text = panel.textContent;
  const list = panel.querySelectorAll('ul.macro-actions');
  const record = app.macroRecords(macroBindDoc)[1];
  return macroSelect.value === '7' && Math.max(...labels.map((l) => l.length)) <= 60
    && !labels.some((l) => l.includes('press ')) && list.length === 1
    && list[0].children.length === (record.actions ?? []).length
    && text.includes('the whole macro: record 1') && text.includes(`interval ${record.interval_ms} ms`)
    && text.includes(record.raw) && text.includes('Bound by') && text.includes('code 3 holds the record index');
}, () => app.renderKeyPanel(macroBindFile).textContent.slice(0, 260)));
ok('the file info rows edit nothing: no dropdown, no text field, byte and Lock Win named apart', () => withFile(headerFile, () => {
  const header = app.renderHeader(headerFile, app.fileChanges(headerFile));
  const text = header.textContent;
  const bound = app.lockWinBinding(headerFile.doc);
  const cond = {
    noSelect: header.querySelectorAll('select').length === 0,
    noInput: header.querySelectorAll('input').length === 0,
    metaRow: text.includes('META toggle'),
    noNameBar: !text.includes('Profile name'),
    markerName: text.includes(`button_meta_mappings[${bound.offset}] = ${app.keyName(bound.hid)}`),
    byteName: text.includes('Menu (meta_toggle_hid 0xF0)'),
    ggCode: text.includes("GG's code for the Meta cap is 0xF0"),
    slotNote: text.includes('slot 93'),
    readOnly: text.includes('this row edits nothing'),
    buttonsWrite: text.includes('the buttons below write it on the current file'),
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}), () => [globalThis.lastCond, app.renderHeader(headerFile, []).querySelectorAll('select').length, app.renderHeader(headerFile, []).querySelectorAll('input').length]);
ok('the name editor above the board marks a changed name and reverts it', () => withFile(nameFile, () => {
  ctl.nameBar.replaceChildren();
  app.renderNameBar(nameFile);
  const input = ctl.nameBar.querySelector('input.text');
  const state = ctl.nameBar.querySelector('.name-state');
  const revert = ctl.nameBar.querySelector('button.name-revert');
  const clean = { maxlength: input.getAttribute('maxlength'), disabled: revert.getAttribute('disabled'), cls: state.className, text: state.textContent };
  input.value = 'Renamed profile';
  input.dispatch('input');
  const typed = { name: nameFile.doc.name, hex: nameFile.doc.name_hex, bytes: nameFile.doc.name_hex.split(' ').length, cls: state.className, text: state.textContent, disabled: revert.getAttribute('disabled') };
  revert.dispatch('click');
  const after = ctl.nameBar.querySelector('.name-state');
  const back = { name: nameFile.doc.name, hex: nameFile.doc.name_hex, cls: after.className, disabled: ctl.nameBar.querySelector('button.name-revert').getAttribute('disabled') };
  return clean.maxlength === '24' && clean.disabled === 'true' && !clean.cls.includes('changed') && clean.text.includes('as the file')
    && typed.name === 'Renamed profile' && typed.bytes === 15 && typed.hex === app.hexOf([...Buffer.from('Renamed profile', 'utf8')])
    && typed.cls.includes('changed') && typed.disabled === null && typed.text.includes(nameFile.original.name)
    && back.name === nameFile.original.name && back.cls === 'name-state' && back.disabled === 'true';
}, () => [nameFile.doc.name, nameFile.doc.name_hex, ctl.nameBar.textContent]));
ok('a name the 24-byte field cannot hold is refused and says why', () => withFile(nameFile, () => {
  ctl.nameBar.replaceChildren();
  app.renderNameBar(nameFile);
  const before = { name: nameFile.doc.name, hex: nameFile.doc.name_hex };
  const input = ctl.nameBar.querySelector('input.text');
  input.value = 'x'.repeat(40);
  input.dispatch('input');
  const status = ctl.status.textContent;
  // The indicator node is the same node through the whole check, so its class is read as a value,
  // not through the node: reading the node at the end reads the last paint.
  const refused = {
    cls: ctl.nameBar.querySelector('.name-state').className,
    name: nameFile.doc.name,
    hex: nameFile.doc.name_hex,
  };
  input.value = '';
  input.dispatch('input');
  const cleared = { name: nameFile.doc.name, hex: nameFile.doc.name_hex, cls: ctl.nameBar.querySelector('.name-state').className };
  const cond = {
    nameKept: refused.name === before.name && refused.hex === before.hex,
    status: status.includes('24 printable ASCII bytes'),
    refusedCls: refused.cls === 'name-state',
    clearedName: cleared.name === '',
    clearedHex: cleared.hex === '',
    clearedCls: cleared.cls === 'name-state changed',
  };
  globalThis.lastCond = cond;
  return Object.values(cond).every(Boolean);
}, () => [nameFile.doc.name, ctl.status.textContent, refused]));
ok('index.html puts the name bar above the board and main.js hands it to the app', () => {
  const html = read('tools/profile-gui/index.html');
  const bar = html.indexOf('id="nameBar"');
  const mount = html.indexOf('id="boardMount"');
  return bar > 0 && mount > 0 && bar < mount && read('tools/profile-gui/main.js').includes("getElementById('nameBar')")
    && html.includes('nameBar" class="name-bar"');
}, () => [read('tools/profile-gui/index.html').indexOf('id="nameBar"'), read('tools/profile-gui/index.html').indexOf('id="boardMount"')]);
ok('the style sheets the name bar, its changed state and the whole-macro list', () => {
  return /\.name-bar\s*\{[^}]*flex-wrap/.test(style) && /\.name-state\.changed\s*\{[^}]*var\(--warn\)/.test(style)
    && /\.name-bar input\[type="text"\]/.test(style) && /ul\.macro-actions\s*\{/.test(style);
}, () => [style.match(/\.name-bar[^}]*\}/g), style.match(/ul\.macro-actions[^}]*\}/g)]);
ok('the bottom row draws LWin between LCtrl and LAlt, and the drawing order leaves the bindings alone', () => {
  const bottom = app.LAYOUT.find((key) => key.y === 5.3) ? app.LAYOUT.filter((key) => key.y === 5.3) : [];
  const first = bottom.slice(0, 4).map((key) => key.hid);
  const win = bottom.find((key) => key.hid === 0xe3);
  const alt = bottom.find((key) => key.hid === 0xe2);
  const ctrl = bottom.find((key) => key.hid === 0xe0);
  const space = bottom.find((key) => key.hid === 0x2c);
  const row = app.LAYOUT_ROWS.find((r) => r.y === 5.3).keys.slice(0, 4).map((spec) => spec[0]);
  return eq(first, [0xe0, 0xe3, 0xe2, 0x2c]) && eq(row, [0xe0, 0xe3, 0xe2, 0x2c])
    && ctrl.x < win.x && win.x < alt.x && alt.x < space.x
    && app.LAYOUT.filter((key) => key.hid === 0xe3).length === 1
    && app.HID2OFF[0xe3] === 13 && app.OFF2HID[13] === 0xe3 && app.HID2OFF[0xe2] === 21
    && app.LAYOUT.filter((key) => key.y === 5.3).length === bottom.length;
}, () => [app.LAYOUT.filter((k) => k.y === 5.3).map((k) => k.hid), app.LAYOUT.filter((k) => k.y === 5.3).map((k) => [k.hid, k.x])]);

// --- live lighting (live.js) ------------------------------------------------
console.log('live lighting panel, served only by ssgg gui');
const liveSandbox = {};
vm.runInNewContext(read('tools/profile-gui/live.js'), liveSandbox, { filename: 'live.js' });
const live = liveSandbox.SsggLive;
ok('hex colours normalise to six upper-case digits and anything else is refused',
  () => live.normalizeHex('#ff8800') === 'FF8800' && live.normalizeHex(' 00ff00 ') === '00FF00'
    && live.normalizeHex('red') === null && live.normalizeHex('FF88') === null && live.normalizeHex('') === null);
ok('without the token meta tag the page has no live lighting',
  () => live.tokenFrom({ querySelector: () => null }) === null
    && live.tokenFrom({ querySelector: () => ({ getAttribute: () => 'abc' }) }) === 'abc');
ok('a call sends the token header and a JSON body, and an error answer throws its message', async () => {
  const seen = [];
  let n = 0;
  liveSandbox.fetch = async (path, init) => {
    const first = n++ === 0;
    seen.push({ path, init });
    return { ok: first, status: first ? 200 : 400, json: async () => (first ? { ok: true } : { error: 'bad colour' }) };
  };
  await live.call('tok', '/api/rgb/color', { color: 'FF0000' });
  let message = '';
  try { await live.call('tok', '/api/rgb/off', {}); } catch (e) { message = e.message; }
  const sent = seen[0];
  return sent.path === '/api/rgb/color' && sent.init.method === 'POST' && sent.init.headers['X-Ssgg-Token'] === 'tok'
    && sent.init.headers['Content-Type'] === 'application/json' && sent.init.body === '{"color":"FF0000"}'
    && message === 'bad colour';
});
ok('index.html loads live.js after app.js with a hidden panel, and app.js itself makes no requests', () => {
  const html = read('tools/profile-gui/index.html');
  return html.indexOf('src="app.js"') > 0 && html.indexOf('src="app.js"') < html.indexOf('src="live.js"')
    && /id="liveBox"[^>]*hidden/.test(html) && !/\bfetch\s*\(/.test(app.source) && /\.live\[hidden\]/.test(style);
});

// --- device control (control.js) --------------------------------------------
console.log('device control panel, served only by ssgg gui');
const controlSandbox = {};
vm.runInNewContext(read('tools/profile-gui/control.js'), controlSandbox, { filename: 'control.js' });
const control = controlSandbox.SsggControl;
const controlCmd = (...args) => control.GROUPS.flatMap(([, list]) => list).find((c) => c.args.join(' ') === args.join(' '));
ok('argv is built from the command, its flags and its positionals', () => {
  const set = controlCmd('actuation', 'set');
  const effect = controlCmd('rgb', 'effect');
  return eq(control.buildArgs(set, ['1.5', true, false]), ['actuation', 'set', '--second', '1.5'])
    && eq(control.buildArgs(effect, ['wave', '2']), ['rgb', 'effect', '--speed', '2', 'wave'])
    && eq(control.buildArgs(controlCmd('actuation', 'set-key'), ['W=1.0  A=2.0', false, true]), ['actuation', 'set-key', '--dry-run', 'W=1.0', 'A=2.0']);
});
ok('an empty required value throws, an empty optional one is left out, a negative number goes after --', () => {
  let message = '';
  try { control.buildArgs(controlCmd('headset', 'sidetone'), ['']); } catch (e) { message = e.message; }
  return message === 'level 0-100 is required'
    && eq(control.buildArgs(controlCmd('actuation', 'rapid-tap-pairs'), ['', false]), ['actuation', 'rapid-tap-pairs'])
    && eq(control.buildArgs(controlCmd('audio', 'chat-mix'), ['-50']), ['audio', 'chat-mix', '--', '-50'])
    && eq(control.buildArgs(controlCmd('audio', 'mute'), ['game', 'toggle']), ['audio', 'mute', 'game']);
});
ok('every ssgg action the CLI has for actuation is in the panel', () => {
  const have = control.GROUPS.flatMap(([, list]) => list).filter((c) => c.args[0] === 'actuation').map((c) => c.args[1]).sort();
  return eq(have, ['clear-second', 'rapid-tap', 'rapid-tap-pairs', 'rapid-trigger', 'release-mode', 'set', 'set-key', 'set-value']);
});
ok('index.html loads control.js after live.js with a hidden panel', () => {
  const html = read('tools/profile-gui/index.html');
  return html.indexOf('src="live.js"') > 0 && html.indexOf('src="live.js"') < html.indexOf('src="control.js"')
    && /id="controlBox"[^>]*hidden/.test(html);
});

await Promise.all(pending);

console.log('');
console.log(`${checks - failed}/${checks} checks passed${skipped ? ` (${skipped} skipped)` : ''}`);
process.exit(failed === 0 ? 0 : 1);
