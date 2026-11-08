import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

class Element {
  constructor() { this.children = []; this.value = ''; this.hidden = false; this.textContent = ''; this.listeners = {}; this.replacements = 0; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; this.replacements++; }
  addEventListener(name, callback) { this.listeners[name] = callback; }
}

test('обновляет результаты и не перерисовывает неизменённый список', async () => {
  const ids = ['#results-list', '#status', '#result-count', '#search', '#sort'];
  const elements = Object.fromEntries(ids.map(id => [id, new Element()]));
  const document = { querySelector: id => elements[id], createElement: () => new Element() };
  let data = []; let tick; let stamp = 1; const calls = [];
  const fetch = async (url, options) => { calls.push({ url, options }); return { ok: true, json: async () => data }; };
  const context = { document, fetch, Intl, Number, String, Array, Error, Date: { now: () => stamp }, setInterval: (fn, delay) => { assert.equal(delay, 60000); tick = fn; } };
  vm.runInNewContext(fs.readFileSync(new URL('../docs/app.js', import.meta.url), 'utf8'), context);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(elements['#result-count'].textContent, '0');
  assert.equal(calls[0].options.cache, 'no-store');
  data = [{ schema: 1, provider: 'Example', cpu_model: 'CPU', region: '', plan: '', vcpu: 2, ram_mb: 2048 }];
  stamp = 2; await tick();
  assert.equal(elements['#result-count'].textContent, '1');
  assert.notEqual(calls[0].url, calls[1].url);
  const rendered = elements['#results-list'].replacements;
  stamp = 3; await tick();
  assert.equal(elements['#results-list'].replacements, rendered);
});
