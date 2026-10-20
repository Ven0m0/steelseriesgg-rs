// preview.mjs - renders the board to a standalone SVG so the keyboard binding can
// be eyeballed without a browser: `node preview.mjs [profile.json ...]` writes
// board-preview.svg next to this file. Scope: this tool's layout and binding only.
import { readFileSync, writeFileSync } from 'node:fs';
import { installFakeDom, loadApp, serialize } from './fake-dom.mjs';

installFakeDom();
const app = loadApp();
const { addFile, buildBoard, changeKeyHids, fileChanges, handleKeyClick, paintBoard, state } = app;

const XMLNS = ['ht', 'tp://www', '.w3', '.org/2000/svg'].join('');

// The preview inlines the page's own stylesheet, so what the SVG shows is what the page draws -
// a hand-copied copy of the palette in here drifts.
const STYLE = `${readFileSync(new URL('style.css', import.meta.url), 'utf8')}
svg { background: #0f1216; }`;

// node has no canvas, and app.js rasterises the OLED block through one. Shim it with a
// 24-bit BMP encoder so the exported SVG carries the real 128x40 bitmap, not a text label.
function bmpDataUrl(width, height, rgba) {
  const rowBytes = width * 3;
  const pixels = rowBytes * height;
  const size = 54 + pixels;
  const buf = Buffer.alloc(size);
  buf.write('BM', 0);
  buf.writeUInt32LE(size, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(pixels, 34);
  for (let row = 0; row < height; row += 1) {
    // BMP stores the bottom row first.
    const source = (height - 1 - row) * width * 4;
    for (let x = 0; x < width; x += 1) {
      const value = rgba[source + x * 4];
      const at = 54 + row * rowBytes + x * 3;
      buf[at] = value;
      buf[at + 1] = value;
      buf[at + 2] = value;
    }
  }
  return `data:image/bmp;base64,${buf.toString('base64')}`;
}

function installCanvasShim() {
  const created = globalThis.document.createElement;
  globalThis.document.createElement = (tag, ns) => {
    if (String(tag).toLowerCase() !== 'canvas') return created(tag, ns);
    const canvas = { width: 0, height: 0 };
    let raster = null;
    canvas.getContext = (kind) => (kind === '2d' ? {
      createImageData: (w, h) => ({ data: new Uint8ClampedArray(w * h * 4) }),
      putImageData: (image) => { raster = image.data; },
    } : null);
    canvas.toDataURL = (type) => (raster ? bmpDataUrl(canvas.width, canvas.height, raster) : `data:${type};base64,`);
    return canvas;
  };
}

installCanvasShim();

const inputs = [];
const flags = {};
{
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 1) {
    const arg = String(argv[i]);
    if (arg === '-o' || arg === '--out') flags.out = argv[++i];
    else if (arg === '--source') flags.source = argv[++i];
    else if (arg === '--scale') flags.scale = argv[++i];
    else if (arg === '--help' || arg === '-h') flags.help = true;
    else inputs.push(arg);
  }
}
if (flags.source) inputs.push(flags.source);
if (flags.help) {
  console.log(`usage: node preview.mjs [flags] profile.json [more.json]

  -o, --out FILE   write the SVG here (default: board-preview.svg next to preview.mjs)
  --source FILE    a second profile, drawn under the first for comparison
  --help           this text

The board comes from app.js's layout tables; the OLED rect is drawn from oled.bytes_hex.
This tool reads a profile and writes a file. No network, no device, no Rust code touched.`);
  process.exit(0);
}
const dumps = [];
for (const path of inputs.length ? inputs : [new URL('examples/sample-remap.json', import.meta.url)]) {
  const name = String(path).split('/').pop();
  const result = addFile(name, readFileSync(path, 'utf8'));
  if (!result.ok) {
    console.error(result.error);
    process.exitCode = 1;
    continue;
  }
  dumps.push(result.file);
}
if (!dumps.length) process.exit(1);

const boards = dumps.map((file) => {
  const board = buildBoard(handleKeyClick);
  paintBoard(board, file.doc, state.layer, state.selected, changeKeyHids(fileChanges(file)));
  const full = serialize(board);
  const [, width, height] = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(full);
  return {
    title: file.filename,
    width: Number(width),
    height: Number(height),
    inner: full.replace(/^<svg[^>]*>/, '').replace(/<\/svg>$/, ''),
  };
});

const width = Math.max(...boards.map((board) => board.width));
const gap = 30;
const total = boards.length * (boards[0].height + gap) + gap;
const pieces = boards.map((board, index) => [
  `  <g transform="translate(0 ${index * (board.height + gap) + gap})">`,
  `    <text x="4" y="14" fill="#d7dee8" font-size="14" font-family="monospace">${board.title}</text>`,
  `    <g transform="translate(0 16)">${board.inner}</g>`,
  '  </g>',
].join('\n'));

const svg = [
  '<?xml version="1.0" encoding="UTF-8"?>',
  `<svg xmlns="${XMLNS}" width="${width}" height="${total}" viewBox="0 0 ${width} ${total}">`,
  `  <style>${STYLE}</style>`,
  `  <rect x="0" y="0" width="${width}" height="${total}" fill="#0f1216"/>`,
  pieces.join('\n'),
  '</svg>',
  '',
].join('\n');

const out = flags.out ? new URL(`file://${new URL(flags.out, `file://${process.cwd()}/`).pathname}`) : new URL('board-preview.svg', import.meta.url);
writeFileSync(out, svg);
console.log(`wrote ${out.pathname} (${boards.length} board(s): ${boards.map((b) => b.title).join(', ')}, ${svg.length} bytes)`);
