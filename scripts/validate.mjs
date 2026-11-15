import net from 'node:net';

const textFields = { provider: 80, region: 80, plan: 80, cpu_model: 100, nmos_version: 32, cpu_benchmark_version: 48, disk_read_method: 32 };
const numericFields = ['cpu_single_ops_per_sec', 'cpu_multi_ops_per_sec', 'steal_percent_load', 'disk_random_iops', 'disk_random_p99_us', 'network_download_mbps', 'sqlite_single_tps', 'sqlite_four_tps', 'sqlite_read_p99_ms'];
const optional = ['region', 'plan', 'cpu_benchmark_version', 'disk_read_method', ...numericFields];
const allowed = new Set(['schema', 'provider', 'region', 'plan', 'tested_at', 'nmos_version', 'cpu_model', 'vcpu', 'ram_mb', ...numericFields, 'cpu_benchmark_version', 'disk_read_method']);
function fail(message) { throw new Error(message); }
function checkText(value, field, max) {
  if (typeof value !== 'string' || [...value].length > max || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) fail(`Проверьте поле «${field}»: недопустимый текст`);
  if (/https?:|www\.|[^\s@]+@[^\s@]+\.[^\s@]+|\\/iu.test(value)) fail(`Уберите ссылку или адрес из поля «${field}»`);
  for (const word of value.split(/[\s,;()[\]]+/u)) if (net.isIP(word) || /(?:\d{1,3}\.){3}\d{1,3}/u.test(word)) fail(`Уберите IP-адрес из поля «${field}»`);
}
export function validateResult(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) fail('Не удалось прочитать результат проверки');
  const data = { ...input };
  if (data.schema !== 1) fail('Версия отчёта не поддерживается');
  for (const key of Object.keys(data)) if (!allowed.has(key)) fail(`В отчёте есть неизвестное поле «${key}»`);
  for (const key of optional) if (data[key] === undefined) data[key] = key === 'region' || key === 'plan' ? '' : key === 'disk_read_method' ? 'not_tested' : null;
  for (const key of allowed) if (!Object.hasOwn(data, key)) fail(`В отчёте отсутствует поле «${key}»`);
  for (const [field, limit] of Object.entries(textFields)) {
    if (field === 'cpu_benchmark_version' && data[field] === null) continue;
    checkText(data[field], field, limit);
  }
  for (const field of ['provider', 'region', 'plan', 'cpu_model', 'nmos_version']) data[field] = data[field].trim();
  if (!data.provider || !data.cpu_model || !data.nmos_version) fail('Укажите провайдера и сведения о процессоре');
  if (typeof data.tested_at !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(data.tested_at) || Number.isNaN(Date.parse(data.tested_at + 'T00:00:00Z')) || new Date(data.tested_at + 'T00:00:00Z').toISOString().slice(0,10) !== data.tested_at) fail('Проверьте дату проверки');
  if (!Number.isInteger(data.vcpu) || data.vcpu < 1 || data.vcpu > 65536 || !Number.isInteger(data.ram_mb) || data.ram_mb < 0 || data.ram_mb > 1073741824) fail('Проверьте количество ядер и объём памяти');
  for (const key of numericFields) if (data[key] !== null && (typeof data[key] !== 'number' || !Number.isFinite(data[key]) || data[key] < 0 || data[key] > 1e12)) fail(`Проверьте показатель «${key}»`);
  if (data.steal_percent_load !== null && data.steal_percent_load > 100) fail('Неверное значение загрузки процессора');
  if (!['O_DIRECT', 'buffered_or_unknown', 'not_tested'].includes(data.disk_read_method)) fail('Неверный способ проверки диска');
  return Object.fromEntries([...allowed].map(key => [key, data[key]]));
}
