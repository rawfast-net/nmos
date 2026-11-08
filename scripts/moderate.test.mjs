import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const row = { schema: 1, provider: 'Example', region: '', plan: '', tested_at: '2026-11-08', nmos_version: '0.3.1', cpu_model: 'Intel(R) CPU @ 2.90GHz', vcpu: 4, ram_mb: 8192, cpu_single_ops_per_sec: 100, cpu_multi_ops_per_sec: 300, cpu_benchmark_version: 'nmos-cpu-1', steal_percent_load: 0, disk_random_iops: null, disk_random_p99_us: null, disk_read_method: 'not_tested', network_download_mbps: null, sqlite_single_tps: null, sqlite_four_tps: null, sqlite_read_p99_ms: null };
const body = value => `Результат\n\n\`\`\`json\n${JSON.stringify(value)}\n\`\`\`\n\nЯ проверил данные и согласен на их публичное размещение.`;

test('проверка и добавление после одобрения', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'nmos-moderation-'));
  try {
    fs.mkdirSync(path.join(root, 'docs', 'data'), { recursive: true });
    const dataPath = path.join(root, 'docs', 'data', 'results.json');
    const eventPath = path.join(root, 'event.json');
    const script = fileURLToPath(new URL('./moderate.mjs', import.meta.url));
    fs.writeFileSync(dataPath, '[]\n');
    const event = { action: 'labeled', label: { name: 'approved' }, issue: { number: 1, body: body(row) } };
    fs.writeFileSync(eventPath, JSON.stringify(event));
    const run = (...args) => spawnSync(process.execPath, [script, eventPath, ...args], { cwd: root, encoding: 'utf8' });
    assert.equal(run('--check').status, 0);
    assert.equal(JSON.parse(fs.readFileSync(dataPath)).length, 0);
    assert.equal(run().status, 0);
    assert.equal(run().status, 0);
    assert.equal(JSON.parse(fs.readFileSync(dataPath)).length, 1);
    event.issue.number = 2;
    fs.writeFileSync(eventPath, JSON.stringify(event));
    assert.notEqual(run().status, 0);
    event.issue.body = body({ ...row, provider: 'person@example.com' });
    fs.writeFileSync(eventPath, JSON.stringify(event));
    assert.notEqual(run().status, 0);
    event.issue.body = body(row); event.label.name = 'pending';
    fs.writeFileSync(eventPath, JSON.stringify(event));
    assert.notEqual(run().status, 0);
    assert.equal(JSON.parse(fs.readFileSync(dataPath)).length, 1);
  } finally {
    const resolved = fs.realpathSync(root);
    if (!resolved.startsWith(fs.realpathSync(os.tmpdir()) + path.sep)) throw new Error('Временный каталог вне указанного пути');
    fs.rmSync(resolved, { recursive: true, force: true });
  }
});
