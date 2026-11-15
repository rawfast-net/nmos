import fs from 'node:fs';
import { validateResult } from './validate.mjs';

const file = 'docs/data/results.json';
const agreements = ['Я проверил данные и согласен на их публичное размещение.', 'Подтверждаю публикацию этих данных.'];
export function extract(body) {
  if (typeof body !== 'string' || body.length > 30000) throw new Error('Отчёт отсутствует или слишком большой');
  if (!agreements.some(value => body.includes(value))) throw new Error('Не найдено согласие на публикацию');
  const blocks = [...body.matchAll(/```json\s*\n([\s\S]*?)\n```/gi)];
  if (blocks.length !== 1) throw new Error('Не удалось найти один отчёт в сообщении');
  let data;
  try { data = JSON.parse(blocks[0][1]); } catch { throw new Error('Отчёт повреждён. Создайте его повторно в NMOS'); }
  return validateResult(data);
}
export function update(existing, data, issue) {
  if (!Array.isArray(existing) || !Number.isSafeInteger(issue) || issue < 1) throw new Error('Не удалось обновить базу результатов');
  if (existing.some(row => row.source_issue === issue)) return { records: existing, added: false };
  const fingerprint = JSON.stringify(data);
  if (existing.some(row => {
    try { const { source_issue, ...value } = row; return JSON.stringify(validateResult(value)) === fingerprint; } catch { return false; }
  })) throw new Error('Такой результат уже опубликован');
  return { records: [...existing, { ...data, source_issue: issue }], added: true };
}
if (process.argv[2]) {
  try {
    const event = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
    if (event.issue?.pull_request || !Number.isSafeInteger(event.issue?.number)) throw new Error('Неподходящее событие');
    const data = extract(event.issue.body);
    if (process.argv[3] === '--check') console.log('Отчёт принят. Ожидается проверка модератором.');
    else {
      if (event.action !== 'labeled' || event.label?.name !== 'approved') throw new Error('Результат не одобрен');
      const result = update(JSON.parse(fs.readFileSync(file, 'utf8')), data, event.issue.number);
      if (result.added) fs.writeFileSync(file, JSON.stringify(result.records, null, 2) + '\n');
      console.log(result.added ? 'Результат опубликован' : 'Этот результат уже опубликован');
    }
  } catch (error) { console.error(`Не удалось обработать результат: ${error.message}`); process.exitCode = 1; }
}
