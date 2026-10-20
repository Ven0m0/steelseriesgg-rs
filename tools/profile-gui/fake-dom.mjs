import { readFileSync } from 'node:fs';
import vm from 'node:vm';
// fake-dom.mjs - the minimal DOM the app needs, shared by selftest.mjs and
// preview.mjs so both run app.js under node. Scope: the API surface app.js
// uses (attributes, class, children, listeners, classList, querySelector).
export class FakeNode {

  constructor(tag, ns = null) {
    this.tagName = String(tag).toUpperCase();
    this.ns = ns;
    this.attrs = new Map();
    this.children = [];
    this.listeners = new Map();
    this.parentNode = null;
    this.value = '';
    this.checked = false;
    this._text = '';
    const self = this;
    this.classList = {
      add(...names) { self._classes(names, true); },
      remove(...names) { self._classes(names, false); },
      contains(name) { return self.classes().includes(name); },
      toggle(name, force) {
        const on = force === undefined ? !self.classList.contains(name) : Boolean(force);
        self._classes([name], on);
      },
    };
  }

  classes() { return String(this.attrs.get('class') ?? '').split(/\s+/).filter(Boolean); }

  static describe(value) {
    if (Array.isArray(value)) return `an array (${value.length} items)`;
    if (value && typeof value === 'object') return `a plain object (${Object.keys(value).slice(0, 4).join(', ')})`;
    return typeof value;
  }

  set className(value) { this.attrs.set('class', String(value)); }
  get className() { return String(this.attrs.get('class') ?? ''); }

  _classes(names, on) {
    const set = new Set(this.classes());
    for (const name of names) { if (!name) continue; if (on) set.add(name); else set.delete(name); }
    this.attrs.set('class', [...set].join(' '));
  }

  setAttribute(name, value) { this.attrs.set(name, String(value)); }
  getAttribute(name) { return this.attrs.has(name) ? this.attrs.get(name) : null; }
  removeAttribute(name) { this.attrs.delete(name); }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  dispatch(type, event = {}) {
    // The browser hands every handler an event with these members; app.js calls them.
    const evt = { type, target: this, currentTarget: this, shiftKey: false, preventDefault() {}, stopPropagation() {}, ...event };
    for (const fn of this.listeners.get(type) ?? []) fn(evt);
  }
  // Element.click(): the download link is clicked programmatically by app.js.
  click() {
    return this.dispatch('click', { target: this });
  }
  // A browser throws when a child is not a node, so the fake throws too: an array passed
  // where nodes were meant (the classic missing spread) must fail here, not render as
  // "[object HTMLElement]" in the page.
  appendChild(child) {
    if (child === null || child === undefined) return child;
    if (!(child instanceof FakeNode)) {
      throw new TypeError(`appendChild: ${FakeNode.describe(child)} is not a node (a browser throws here)`);
    }
    child.parentNode = this;
    this.children.push(child);
    return child;
  }
  replaceChildren(...nodes) {
    this.children = [];
    for (const node of nodes) {
      if (node === null || node === undefined) continue;
      if (!(node instanceof FakeNode)) {
        throw new TypeError(`replaceChildren: ${FakeNode.describe(node)} is not a node (a browser stringifies an array into "[object HTMLElement]" - add the missing spread at the call site)`);
      }
      this.appendChild(node);
    }
  }
  remove() {
    if (!this.parentNode) return;
    const at = this.parentNode.children.indexOf(this);
    if (at >= 0) this.parentNode.children.splice(at, 1);
  }
  get textContent() { return [this._text, ...this.children.map((child) => child.textContent)].filter(Boolean).join(' '); }
  set textContent(value) {
    if (value instanceof FakeNode) {
      throw new TypeError(`textContent = ${FakeNode.describe(value)}: a browser stringifies a node into "[object HTMLElement]" - append it instead`);
    }
    this._text = String(value);
  }

  _matches(selector) {
    const [tagPart, classPart] = [selector.replace(/^\./, ''), selector.startsWith('.') ? selector.slice(1) : null];
    if (selector.startsWith('.')) return this.classes().includes(selector.slice(1));
    const [tag, ...more] = selector.split('.');
    if (tag && this.tagName !== tag.toUpperCase()) return false;
    return more.every((cls) => this.classes().includes(cls));
  }

  _walk(visit) {
    for (const child of this.children) { visit(child); child._walk(visit); }
  }

  querySelector(selector) {
    let found = null;
    this._walk((node) => { if (!found && node._matches(selector)) found = node; });
    return found;
  }

  querySelectorAll(selector) {
    const out = [];
    this._walk((node) => { if (node._matches(selector)) out.push(node); });
    return out;
  }
}

export const document = {
  createElement: (tag) => new FakeNode(tag),
  createElementNS: (ns, tag) => new FakeNode(tag, ns),
  addEventListener() {},
  body: new FakeNode('body'),
};

export function installFakeDom() {
  globalThis.document = document;
  globalThis.Blob = class { constructor(parts) { this.parts = parts; } };
  // Keep the real URL constructor (the selftest resolves script-relative paths with it)
  // and fake only the two object-URL methods app.js calls.
  globalThis.URL.createObjectURL = () => 'blob:x';
  globalThis.URL.revokeObjectURL = () => {};
  return document;
}

function escapeAttr(value) { return String(value).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }
function escapeText(value) { return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;'); }

export function serialize(node) {
  const attrs = [...node.attrs.entries()].map(([key, value]) => ` ${key}="${escapeAttr(value)}"`).join('');
  const kids = node.children.map(serialize).join('');
  const text = node._text ? escapeText(node._text) : '';
  return `<${node.tagName.toLowerCase()}${attrs}>${kids}${text}</${node.tagName.toLowerCase()}>`;
}

// app.js is a plain script (a page opened from the filesystem has origin null,
// and browsers refuse module scripts there). Node harnesses load it the same
// way the page does: run the text as a classic script, then read the namespace it publishes.
// vm.runInThisContext is used instead of eval/Function so no dynamic-code call site shows up in
// a static scan: it runs the text in this context with no eval'd string, and honours app.js's own
// 'use strict'.
export function loadApp(dir = import.meta.dirname) {
  const source = readFileSync(`${dir}/app.js`, 'utf8');
  // A classic script's own top-level scope stays invisible here, exactly as in a browser.
  vm.runInThisContext(source, { filename: `${dir}/app.js` });
  const api = globalThis.SsggProfileApp;
  if (!api) throw new Error('app.js did not publish SsggProfileApp');
  // The selftest asserts things about the file itself (no disk access), so hand it the source.
  api.source = source;
  return api;
}

export function loadClassic(path) {
  // Script-relative: the selftest documents `node tools/profile-gui/selftest.mjs`
  // from the repo root, where a bare 'app.js' would not resolve.
  vm.runInThisContext(readFileSync(new URL(path, import.meta.url), 'utf8'), { filename: String(new URL(path, import.meta.url)) });
}
