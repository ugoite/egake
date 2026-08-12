#!/usr/bin/env node

// Focus and refresh behavior needs a DOM, but does not need a browser or pixel snapshots.
// This deliberately tiny DOM harness exercises the dependency-free runtime contract.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

class Element {
  constructor(tagName, ownerDocument) {
    this.tagName = tagName.toUpperCase();
    this.ownerDocument = ownerDocument;
    this.children = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.listeners = new Map();
    this.hidden = false;
    this.value = "";
    this._textContent = "";
  }

  get id() { return this.getAttribute("id") || ""; }
  set id(value) { this.setAttribute("id", value); }
  get firstChild() { return this.children[0] || null; }
  get textContent() { return this._textContent; }
  set textContent(value) {
    this._textContent = String(value ?? "");
    this.children.forEach((child) => { child.parentNode = null; });
    this.children = [];
  }

  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getAttribute(name) { return this.attributes.has(name) ? this.attributes.get(name) : null; }
  removeAttribute(name) { this.attributes.delete(name); }
  appendChild(child) {
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.children.push(child);
    return child;
  }
  append(...children) { children.forEach((child) => this.appendChild(child)); }
  removeChild(child) {
    const index = this.children.indexOf(child);
    if (index >= 0) this.children.splice(index, 1);
    child.parentNode = null;
    if (this.ownerDocument && this.ownerDocument.activeElement === child) this.ownerDocument.activeElement = this.ownerDocument.body;
    return child;
  }
  replaceChildren(...children) {
    if (this.ownerDocument && this.ownerDocument.activeElement && this.contains(this.ownerDocument.activeElement)) this.ownerDocument.activeElement = this.ownerDocument.body;
    this.children.forEach((child) => { child.parentNode = null; });
    this.children = [];
    children.forEach((child) => this.appendChild(child));
  }
  addEventListener(type, listener) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(listener);
  }
  dispatchEvent(event) {
    (this.listeners.get(event.type) || []).forEach((listener) => listener(event));
  }
  click() {
    this.focus();
    (this.listeners.get("click") || []).forEach((listener) => listener({ target: this }));
  }
  focus() { this.ownerDocument.activeElement = this; }
  contains(node) {
    return this.children.some((child) => child === node || child.contains(node));
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  querySelectorAll(selector) {
    const result = [];
    const visit = (node) => {
      node.children.forEach((child) => {
        if (matches(child, selector)) result.push(child);
        visit(child);
      });
    };
    visit(this);
    return result;
  }
}

function matches(element, selector) {
  return selector.split(",").some((part) => {
    let value = part.trim();
    const notHidden = value.includes(":not([hidden])");
    value = value.replace(":not([hidden])", "");
    if (notHidden && element.hidden) return false;
    const attrs = [...value.matchAll(/\[([\w-]+)(?:=(["']?)([^\]"']+)\2)?\]/g)];
    for (const [, name, , expected] of attrs) {
      const actual = element.getAttribute(name);
      if (actual === null || (expected !== undefined && actual !== expected)) return false;
    }
    value = value.replace(/\[[^\]]+\]/g, "");
    const ids = [...value.matchAll(/#([\w-]+)/g)];
    if (ids.some(([, id]) => element.id !== id)) return false;
    const classes = [...value.matchAll(/\.([\w-]+)/g)].map(([, name]) => name);
    const classNames = (element.getAttribute("class") || "").split(/\s+/).filter(Boolean);
    if (classes.some((name) => !classNames.includes(name))) return false;
    const tag = value.replace(/[#.].*$/, "").trim();
    return !tag || tag === "*" || element.tagName.toLowerCase() === tag.toLowerCase();
  });
}

class Document extends Element {
  constructor() {
    super("document", null);
    this.ownerDocument = this;
    this.body = new Element("body", this);
    this.appendChild(this.body);
    this.activeElement = this.body;
    this.title = "";
  }
  createElement(tagName) { return new Element(tagName, this); }
  getElementById(id) {
    return this.querySelectorAll("[id]").find((element) => element.id === id) || null;
  }
}

const document = new Document();
const root = document.createElement("div");
root.id = "egake-root";
document.body.appendChild(root);
const inline = document.createElement("script");
inline.id = "egake-application";
inline.textContent = JSON.stringify({
  profile: { name: "Runtime test", version: "0.1" },
  resources: [{ name: "tasks", capabilities: ["list"], fields: [{ name: "id" }, { name: "title" }] }],
  states: [],
  pages: [{ title: "Tasks", components: [
    { kind: "row", children: [
      { kind: "button", text: "New", attributes: { action: "open-create" } },
      { kind: "button", text: "New", attributes: { action: "open-create" } },
    ] },
    { kind: "data-table", attributes: { resource: "tasks", key: "id" }, children: [
      { kind: "column", attributes: { field: "id", label: "ID" } },
    ] },
    { kind: "form", id: "editor", attributes: { mode: "drawer" }, children: [
      { kind: "text-input", id: "title", attributes: { field: "title", label: "Title" } },
      { kind: "button", text: "Save", attributes: { action: "save" } },
      { kind: "button", text: "Delete", attributes: { action: "delete-task" } },
    ] },
    { kind: "form", id: "alternate-editor", attributes: { mode: "drawer" }, children: [] },
  ] }],
  actions: [{ name: "open-create", steps: [] }, { name: "save", steps: [] }, { name: "delete-task", steps: [] }],
});
document.body.appendChild(inline);

let deferredListResolve;
let deferNextList = false;
let deleted = false;
const response = (body) => ({ ok: true, status: 200, text: () => Promise.resolve(JSON.stringify(body)) });
const window = {
  document,
  __EGAKE_TEST__: {},
  confirm: () => true,
  setTimeout,
  clearTimeout,
  fetch(url, options) {
    if (url.includes("/schema")) return Promise.resolve(response({ fields: [{ name: "id" }, { name: "title" }] }));
    if (options && options.method === "DELETE") {
      deleted = true;
      return Promise.resolve(response({}));
    }
    if (deferNextList) {
      deferNextList = false;
      return new Promise((resolve) => { deferredListResolve = () => resolve(response({ items: [{ id: "a", title: "A" }] })); });
    }
    return Promise.resolve(response({ items: deleted ? [] : [{ id: "a", title: "A" }] }));
  },
};

const context = { window, document, URLSearchParams, Promise, console, setTimeout, clearTimeout };
vm.runInNewContext(fs.readFileSync("crates/egake-cli/assets/runtime.js", "utf8"), context, { filename: "runtime.js" });
const hooks = window.__EGAKE_TEST__;
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

(async () => {
  await hooks.ready;
  assert.equal(document.querySelector(".egake-table").getAttribute("aria-busy"), "false");

  const addButtons = document.querySelectorAll('[data-action="open-create"]');
  assert.notEqual(addButtons[0].getAttribute("data-focus-key"), addButtons[1].getAttribute("data-focus-key"), "duplicate anonymous openers have distinct keys");
  addButtons[0].click();
  await tick();
  assert.equal(document.activeElement.id, "title", "opening an editor focuses its first control");
  hooks.render();
  await tick();
  assert.equal(document.activeElement.id, "title", "an editor input keeps focus across rerenders");
  hooks.closeEditor();
  await tick();
  assert.equal(document.activeElement.getAttribute("data-focus-key"), addButtons[0].getAttribute("data-focus-key"), "close restores the exact opener");

  const secondAddButton = document.querySelectorAll('[data-action="open-create"]')[1];
  secondAddButton.click();
  await tick();
  hooks.closeEditor();
  await tick();
  assert.equal(document.activeElement.getAttribute("data-focus-key"), secondAddButton.getAttribute("data-focus-key"), "stable keys distinguish duplicate action names");

  const row = document.querySelector("[data-record-key]");
  row.click();
  await tick();
  hooks.closeEditor();
  await tick();
  assert.equal(document.activeElement.getAttribute("data-record-key"), "a", "row opener restores by resource and key");

  row.focus();
  row.dispatchEvent({ type: "keydown", key: "Enter", preventDefault() {} });
  await tick();
  assert.equal(document.activeElement.id, "title", "keyboard row selection opens the editor");
  hooks.closeEditor();
  await tick();

  hooks.model.app.pages[0].components[2].attributes.mode = "dialog";
  hooks.model.state.editorOpen = true;
  hooks.render();
  await tick();
  const titles = document.querySelectorAll(".egake-form-title");
  assert.equal(new Set(titles.map((title) => title.id)).size, titles.length, "form title IDs are unique");
  assert.equal(document.querySelector(".egake-form[data-mode=dialog]").getAttribute("role"), "dialog");
  assert.equal(document.querySelector(".egake-form[data-mode=dialog]").getAttribute("aria-modal"), "true");
  assert.ok(document.querySelector(".egake-backdrop"));
  document.dispatchEvent({ type: "keydown", key: "Escape", preventDefault() {} });
  await tick();
  assert.equal(hooks.model.state.editorOpen, false, "Escape closes a dialog");
  hooks.model.app.pages[0].components[2].attributes.mode = "drawer";

  row.click();
  await tick();
  document.querySelector('[data-action="delete-task"]').click();
  await tick();
  await tick();
  assert.equal(deleted, true, "delete sends the DELETE request");
  assert.equal(document.querySelector("[data-record-key]"), null, "delete refreshes records");
  assert.equal(hooks.model.state.editorOpen, false, "delete closes the editor");
  assert.equal(document.activeElement.getAttribute("class"), "egake-table", "deleted row falls back to its resource table");

  hooks.model.records.tasks = [{ id: "a", title: "A" }];
  hooks.render();
  const table = document.querySelector(".egake-table");
  const preservedRow = document.querySelector("[data-record-key]");
  deferNextList = true;
  const refresh = hooks.refresh("tasks");
  assert.equal(document.querySelector(".egake-table"), table, "refresh keeps the table mounted");
  assert.equal(document.querySelector("[data-record-key]"), preservedRow, "refresh keeps rows mounted while waiting");
  assert.equal(table.getAttribute("aria-busy"), "true");
  deferredListResolve();
  await refresh;
  assert.equal(document.querySelector(".egake-table").getAttribute("aria-busy"), "false");
  const refreshButton = document.querySelectorAll(".egake-button").find((button) => button.textContent === "Refresh");
  refreshButton.focus();
  hooks.render();
  await tick();
  assert.equal(document.activeElement.textContent, "Refresh", "an anonymous toolbar control keeps focus across rerenders");
  console.log("runtime UI behavior tests passed");
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
