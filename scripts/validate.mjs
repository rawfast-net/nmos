import net from 'node:net';

const textFields = { provider: 80, region: 80, plan: 80, cpu_model: 100, nmos_version: 32, cpu_benchmark_version: 48, disk_read_method: 32 };
const numericFields = ['cpu_single_ops_per_sec', 'cpu_multi_ops_per_sec', 'steal_percent_load', 'disk_random_iops', 'disk_random_p99_us', 'network_download_mbps', 'sqlite_single_tps', 'sqlite_four_tps', 'sqlite_read_p99_ms'];
const required = ['schema', 'provider', 'region', 'plan', 'tested_at', 'nmos_version', 'cpu_model', 'vcpu', 'ram_mb', ...numericFields, 'cpu_benchmark_version', 'disk_read_method'];

function fail(message) { throw new Error(message); }
function checkText(value, field, max) {
  if (typeof value !== 'string' || [...value].length > max || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) fail(`Недопустимое поле ${field}`);
  if (/https?:|www\.|[^\s@]+@[^\s@]+\.[^\s@]+|\\/iu.test(value)) fail(`Ссылка или адрес в поле ${field}`);
  for (const word of value.split(/[\s,;()[\]]+/u)) if (net.isIP(word) || /(?:\d{1,3}\.){3}\d{1,3}/u.test(word)) fail(`Адрес в поле ${field}`);
}

export function validateResult(data) {
  if (!data || typeof data !== 'object' || Array.isArray(data)) fail('Ожидается объект');
  if (Object.keys(data).length !== required.length || required.some(key => !Object.hasOwn(data, key))) fail('Неизвестные или отсутствующие поля');
  if (data.schema !== 1) fail('Неподдерживаемая версия схемы');
  for (const [field, limit] of Object.entries(textFields)) {
    if (field === 'cpu_benchmark_version' && data[field] === null) continue;
    checkText(data[field], field, limit);
  }
  if (!data.provider.trim() || !data.cpu_model.trim() || !data.nmos_version.trim()) fail('Пустое обязательное поле');
  if (!/^\d{4}-\d{2}-\d{2}$/.test(data.tested_at) || Number.isNaN(Date.parse(data.tested_at + 'T00:00:00Z')) || new Date(data.tested_at + 'T00:00:00Z').toISOString().slice(0,10) !== data.tested_at) fail('Некорректная дата');
  if (!Number.isInteger(data.vcpu) || data.vcpu < 1 || data.vcpu > 65536 || !Number.isInteger(data.ram_mb) || data.ram_mb < 0 || data.ram_mb > 1073741824) fail('Некорректные характеристики');
  for (const key of numericFields) if (data[key] !== null && (typeof data[key] !== 'number' || !Number.isFinite(data[key]) || data[key] < 0 || data[key] > 1e12)) fail(`Некорректное поле ${key}`);
  if (data.steal_percent_load !== null && data.steal_percent_load > 100) fail('Некорректный steal');
  if (!['O_DIRECT', 'buffered_or_unknown', 'not_tested'].includes(data.disk_read_method)) fail('Некорректный метод чтения');
  return Object.fromEntries(required.map(key => [key, data[key]]));
}

