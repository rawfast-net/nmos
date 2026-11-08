import fs from 'node:fs';
import { validateResult } from './validate.mjs';

const consent = 'Я проверил данные и согласен на их публичное размещение.';
const file = 'docs/data/results.json';
export function extract(body) {
  if (typeof body !== 'string' || body.length > 30000 || !body.includes(consent)) throw new Error('Не найдено согласие автора');
  const blocks = [...body.matchAll(/```json\s*\n([\s\S]*?)\n```/g)];
  if (blocks.length !== 1) throw new Error('Нужен один блок JSON');
  return validateResult(JSON.parse(blocks[0][1]));
}
export function update(existing, data, issue) {
  if (!Array.isArray(existing) || !Number.isSafeInteger(issue) || issue < 1) throw new Error('Некорректная база или Issue');
  if (existing.some(row => row.source_issue === issue)) return { records: existing, added: false };
  const fingerprint = JSON.stringify(data);
  if (existing.some(row => {
    try { const { source_issue, ...value } = row; return JSON.stringify(validateResult(value)) === fingerprint; } catch { return false; }
  })) throw new Error('Результат уже опубликован');
  return { records: [...existing, { ...data, source_issue: issue }], added: true };
}
if (process.argv[2]) {
  try {
    const event = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
    if (event.issue?.pull_request || !Number.isSafeInteger(event.issue?.number)) throw new Error('Неподходящее событие');
    const data = extract(event.issue.body);
    if (process.argv[3] === '--check') { console.log('Формат проверен; требуется решение модератора'); }
    else {
      if (event.action !== 'labeled' || event.label?.name !== 'approved') throw new Error('Нет метки approved');
      const result = update(JSON.parse(fs.readFileSync(file, 'utf8')), data, event.issue.number);
      if (result.added) fs.writeFileSync(file, JSON.stringify(result.records, null, 2) + '\n');
      console.log(result.added ? 'Результат добавлен' : 'Этот Issue уже обработан');
    }
  } catch (error) { console.error(`Результат не обработан: ${error.message}`); process.exitCode = 1; }
}
