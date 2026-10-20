#!/usr/bin/env node
// Import the product ID table from Rust into the profile GUI.
//
// The page encodes and decodes one payload layout, and only some models are on the protocol this
// crate read back, so it needs both the PID -> model name mapping and a per-model feature set. They
// live in Rust (`src/devices/mod.rs`, the class table in `src/devices/discovery.rs` and the codec
// registry in `src/devices/keyboards/onboard.rs`); this script parses those
// source regions and writes `products.generated.js` beside it, which `index.html` loads before
// `app.js`. The page keeps a hand-mirrored copy as a fallback so opening `index.html` in a checkout
// where the generator was never run still shows names.
//
// The product ID is also what the page keys its encodings on: the payload layout, the wheel keys,
// the OLED block and the encodable fields are all per model, so a document naming a model outside
// the family this page implements is labelled as such instead of being encoded with the wrong table.
//
// Run `node tools/profile-gui/sync-products.mjs` after changing the Rust tables, and
// `node tools/profile-gui/sync-products.mjs --check` to fail on drift without rewriting anything
// (`selftest.mjs` runs the check). No dependencies.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const HERE = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(HERE, '..', '..');
export const SOURCES = [
  'src/devices/mod.rs',
  'src/devices/discovery.rs',
  'src/devices/keyboards/onboard.rs',
];
export const OUT = join(HERE, 'products.generated.js');

/** FNV-1a 64-bit hash in hex, the same checksum `ssgg onboard decode` prints. */
export function fnv1a64Hex(text) {
  const ENOUGH = (1n << 53n) - 1n;
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash = ((hash ^ BigInt(byte)) * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return `${hash <= ENOUGH ? Number(hash).toString(16) : hash.toString(16).padStart(16, '0')}`;
}

/** Body of a `pub const NAME ... = [ ... ];` or `= { ... };` block, or `null` when absent. */
function constBlock(source, header) {
  const start = source.indexOf(header);
  if (start < 0) return null;
  const openList = source.indexOf('[', start);
  const openBrace = source.indexOf('{', start);
  const open = openList < 0 ? openBrace : openBrace < 0 ? openList : Math.min(openList, openBrace);
  if (open < 0) return null;
  const close = source.indexOf(source[open] === '[' ? '\n]' : '\n}', open);
  return close < 0 ? null : source.slice(open + 1, close);
}

/** Body of a function starting at `header`, up to the line that closes it. */
function fnBody(source, header) {
  const start = source.indexOf(header);
  if (start < 0) return null;
  const open = source.indexOf('{', start);
  const close = source.indexOf('\n}\n', open);
  return open < 0 || close < 0 ? null : source.slice(open + 1, close);
}

/** `pub const APEX_7: u16 = 0x1612;` entries in the product ID module. */
export function productIds(block) {
  return [...block.matchAll(/pub\s+const\s+([A-Z0-9_]+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+)\s*;/g)].map((m) => ({
    name: m[1],
    pid: Number.parseInt(m[2], 16),
    hex: hexPid(Number.parseInt(m[2], 16)),
  }));
}

/**
 * Walk the arms of a Rust `match`, resolving each arm's pattern to product IDs.
 *
 * A pattern is a `|` group of constant names (`APEX_7 | APEX_7_TKL`), and the arm that matches
 * everything is `_`. Values are the text on the right of `=>`, as `valueSource` captures it.
 */
export function arms(body, byName, valueSource) {
  const re = new RegExp(`(?<pattern>[^=>]*?)\\s*=>\\s*${valueSource}`, 'g');
  const out = {};
  let fallback = null;
  for (const m of body.matchAll(re)) {
    const pattern = m.groups.pattern.replace(/^[^A-Za-z0-9_]*,\s*/u, '');
    const names = [...pattern.matchAll(/[A-Za-z_][A-Za-z0-9_]*/g)].map((x) => x[0]).filter((n) => byName.has(n));
    if (names.length === 0) {
      if (!/^_\s*$/u.test(pattern.trim()))
        throw new Error(`an arm is neither a product list nor a wildcard: ${pattern.trim().slice(0, 60)}`);
      fallback = m.groups.value;
      continue;
    }
    for (const name of names) out[byName.get(name).hex] = m.groups.value;
  }
  return { out, fallback };
}

/** `APEX_7 => "Apex 7",` arms of the name function, resolved through the constant names. */
export function productNames(body, ids) {
  const { out, fallback } = arms(body, new Map(ids.map((e) => [e.name, e])), '(?<value>"[^"]*")');
  const unquote = (text) => (typeof text === 'string' ? text.slice(1, -1) : null);
  const names = Object.fromEntries(Object.entries(out).map(([hex, value]) => [hex, unquote(value)]));
  return { names, fallback: fallback === null ? null : unquote(fallback) };
}

/**
 * Product IDs the arm of a class function binds to one class - by default the `KeyboardClass::Apex7`
 * arm of `keyboard_class` in `src/devices/discovery.rs`, which is the on-board family: it is the only
 * class whose keyboard answers `Keyboard::supports_onboard_profile`, and that is what lets
 * `ssgg onboard read` reach a board.
 *
 * Handles both ways the Rust tables name a constant: `product_ids::APEX_7`, and a bare `APEX_7` in a
 * function with `use product_ids::*;`. Comments are stripped first, so a note naming a constant is
 * not read as a family member.
 */
export function familySet(body, ids, arm = 'KeyboardClass::Apex7') {
  const byName = new Map(ids.map((e) => [e.name, e]));
  const source = body.replace(/^\s*\/\/.*$/gm, '');
  const out = [];
  let from = 0;
  for (const m of source.matchAll(/=>\s*([A-Za-z0-9_:]+)/g)) {
    const pattern = source.slice(from, m.index);
    from = m.index + m[0].length;
    if (m[1] !== arm) continue;
    for (const id of pattern.matchAll(/\b([A-Z][A-Z0-9_]{2,})\b/g)) {
      const entry = byName.get(id[1]);
      if (entry && !out.includes(entry.hex)) out.push(entry.hex);
    }
  }
  return out;
}

/** `APEX_7 => DeviceType::Keyboard` arms, resolved through the constant names, lowercased. */
export function productKinds(body, ids) {
  const { out, fallback } = arms(body, new Map(ids.map((e) => [e.name, e])), 'DeviceType::(?<value>[A-Za-z]+)');
  const lower = (x) => (x === null || x === undefined ? null : x.toLowerCase());
  return { kinds: Object.fromEntries(Object.entries(out).map(([k, v]) => [k, lower(v)])), fallback: lower(fallback) };
}

/** Family codec modules a build registers, as `codecs()` lists them. */
export function codecModules(body) {
  return [...body.matchAll(/keyboards::([a-z0-9_]+)::([A-Za-z0-9_]+)/g)].map((m) => ({ module: m[1], codec: m[2] }));
}

function hexPid(pid) {
  if (!Number.isInteger(pid) || pid < 0 || pid > 0xffff) return null;
  return `0x${pid.toString(16).toUpperCase().padStart(4, '0')}`;
}

/** Everything the page needs out of the Rust sources. */
export function collect(repo = REPO) {
  const read = (rel) => readFileSync(join(repo, rel), 'utf8');
  const mod = read('src/devices/mod.rs');
  const discovery = read('src/devices/discovery.rs');
  const onboard = read('src/devices/keyboards/onboard.rs');

  const idsBlock = constBlock(mod, 'pub mod product_ids');
  const namesBody = fnBody(mod, 'pub fn device_name_from_product_id');
  const familyBody = fnBody(discovery, 'pub fn keyboard_class');
  const typeBody = fnBody(mod, 'pub fn device_type_from_product_id');
  const codecsBody = fnBody(onboard, 'pub fn codecs');
  if (!idsBlock || !namesBody || !familyBody || !typeBody || !codecsBody) {
    throw new Error(
      'the tables this script imports are not where it expects: pub mod product_ids and device_name_from_product_id ' +
        'in src/devices/mod.rs, keyboard_class in src/devices/discovery.rs and codecs in ' +
        'src/devices/keyboards/onboard.rs',
    );
  }

  const ids = productIds(idsBlock);
  const { names, fallback: nameFallback } = productNames(namesBody, ids);
  const onboardPids = familySet(familyBody, ids);
  const { kinds, fallback: kindFallback } = productKinds(typeBody, ids);
  const codecs = codecModules(codecsBody);

  const products = {};
  for (const entry of ids) {
    products[entry.hex] = {
      name: names[entry.hex] ?? null,
      kind: kinds[entry.hex] ?? kindFallback,
      onboard: onboardPids.includes(entry.hex),
    };
  }

  return {
    generatedFrom: SOURCES,
    sourceHash: fnv1a64Hex(SOURCES.map(read).join('\n')),
    productCount: ids.length,
    namedCount: Object.keys(names).length,
    nameFallback,
    onboardPids,
    codecs,
    kindFallback,
    products,
  };
}

/** Text of the generated file: a global the page reads, with the header naming its source. */
export function render(data) {
  const rows = Object.entries(data.products)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([hex, info]) => `    ${JSON.stringify(hex)}: ${JSON.stringify(info)},`);
  return `// Generated by tools/profile-gui/sync-products.mjs -- do not edit by hand.
//
// The product ID -> model name mapping and the per-model on-board profile feature set, imported from
// ${data.generatedFrom.join(', ')}. Regenerate with \`node tools/profile-gui/sync-products.mjs\`;
// \`--check\` (and \`selftest.mjs\`) fails when Rust moved and this file did not.
globalThis.SsggProducts = {
  generatedFrom: ${JSON.stringify(data.generatedFrom)},
  sourceHash: ${JSON.stringify(data.sourceHash)},
  productCount: ${data.productCount},
  namedCount: ${data.namedCount},
  nameFallback: ${JSON.stringify(data.nameFallback)},
  onboardPids: ${JSON.stringify(data.onboardPids)},
  kindFallback: ${JSON.stringify(data.kindFallback)},
  codecs: ${JSON.stringify(data.codecs)},
  products: {
${rows.join('\n')}
  },
};
`;
}

export function main({ argv = process.argv.slice(2), out = OUT, repo = REPO, log = console.log } = {}) {
  const check = argv.includes('--check');
  const data = collect(repo);
  const text = render(data);
  if (check) {
    if (!existsSync(out)) {
      log(`FAIL  ${out} does not exist; run node tools/profile-gui/sync-products.mjs`);
      return 1;
    }
    const current = readFileSync(out, 'utf8');
    if (current !== text) {
      log(`FAIL  ${out} is stale: Rust moved and the generated file did not (source hash ${JSON.stringify(data.sourceHash)})`);
      return 1;
    }
    log(`ok    ${out} is current (source hash ${data.sourceHash}, ${data.productCount} product IDs, ${data.onboardPids.length} on-board models, ${data.codecs.length} codecs)`);
    return 0;
  }
  writeFileSync(out, text);
  log(`wrote ${out} (${data.productCount} product IDs, ${data.namedCount} names, on-board: ${data.onboardPids.join(' ')}, source hash ${data.sourceHash})`);
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  process.exitCode = main();
}
