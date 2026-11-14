// Page entry point: mount the board, hand the controls to app.js.
const { buildBoard, handleKeyClick, init, state } = globalThis.SsggProfileApp;

const board = buildBoard(handleKeyClick);
document.getElementById('boardMount').appendChild(board);

const controls = {
  board,
  boardBox: document.getElementById('boardBox'),
  nameBar: document.getElementById('nameBar'),
  inspector: document.getElementById('inspector'),
  below: document.getElementById('belowBox'),
  tabs: document.getElementById('tabs'),
  legend: document.getElementById('legend'),
  status: document.getElementById('status'),
  fileInput: document.getElementById('fileInput'),
  saveBtn: document.getElementById('saveBtn'),
  layerBaseBtn: document.getElementById('layerBaseBtn'),
  layerMetaBtn: document.getElementById('layerMetaBtn'),
  onRender: syncLayerButtons,
};

// The toolbar layer buttons must match the state every panel reads.
function syncLayerButtons() {
  controls.layerBaseBtn.classList.toggle('active', state.layer === 'base');
  controls.layerMetaBtn.classList.toggle('active', state.layer === 'meta');
}

// Technical details (hex, offsets, protocol notes) are hidden unless this is on; the choice is remembered.
const devBtn = document.getElementById('devBtn');
function setDev(on) {
  document.body.classList.toggle('show-dev', on);
  devBtn.setAttribute('aria-pressed', String(on));
  try { globalThis.localStorage.setItem('ssgg-dev', on ? '1' : '0'); } catch { /* storage may be blocked */ }
}
devBtn.addEventListener('click', () => setDev(!document.body.classList.contains('show-dev')));
try { if (globalThis.localStorage.getItem('ssgg-dev') === '1') setDev(true); } catch { /* storage may be blocked */ }

init(controls);
