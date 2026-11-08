const list = document.querySelector('#results-list');
const status = document.querySelector('#status');
const count = document.querySelector('#result-count');
const search = document.querySelector('#search');
const sort = document.querySelector('#sort');
let results = [];

function element(tag, text, className) {
  const item = document.createElement(tag);
  if (text !== undefined) item.textContent = text;
  if (className) item.className = className;
  return item;
}
function number(value, suffix = '') {
  return typeof value === 'number' && Number.isFinite(value) ? `${new Intl.NumberFormat('ru-RU', {maximumFractionDigits: 1}).format(value)}${suffix}` : '—';
}
function metric(label, value) {
  const item = element('div', undefined, 'metric');
  item.append(element('strong', value), element('span', label));
  return item;
}
function render() {
  const query = search.value.trim().toLocaleLowerCase('ru');
  const filtered = results.filter(r => [r.provider, r.region, r.plan, r.cpu_model].some(x => String(x || '').toLocaleLowerCase('ru').includes(query)));
  filtered.sort((a, b) => sort.value === 'provider' ? a.provider.localeCompare(b.provider, 'ru') : sort.value === 'cpu' ? (a.cpu_model || '').localeCompare(b.cpu_model || '', 'ru') : (b.tested_at || '').localeCompare(a.tested_at || ''));
  count.textContent = String(results.length);
  list.replaceChildren();
  status.hidden = filtered.length > 0;
  status.textContent = results.length ? 'По вашему запросу ничего не найдено.' : 'Пока нет опубликованных результатов. Вы можете добавить первый после проверки своего сервера.';
  for (const r of filtered) {
    const card = element('article', undefined, 'result');
    const identity = element('div');
    identity.append(element('h3', r.provider), element('span', [r.region, r.plan, r.tested_at].filter(Boolean).join(' · '), 'sub'));
    const cpu = element('div', undefined, 'metric');
    cpu.append(element('strong', r.cpu_model || 'Модель не указана'), element('span', `${number(r.vcpu)} vCPU · ${number(r.ram_mb / 1024, ' GiB RAM')}`));
    const details = element('details');
    const summary = element('summary', 'Условия и все показатели');
    const data = element('div', undefined, 'detail-grid');
    for (const [label, value] of [
      ['Тест CPU', r.cpu_benchmark_version || 'не проводился'],
      ['CPU, 1 поток', number(r.cpu_single_ops_per_sec, ' оп/с')],
      ['CPU, все потоки', number(r.cpu_multi_ops_per_sec, ' оп/с')],
      ['CPU steal под нагрузкой', number(r.steal_percent_load, '%')],
      ['Random read', number(r.disk_random_iops, ' IOPS')],
      ['Random read p99', number(r.disk_random_p99_us, ' мкс')],
      ['Метод чтения', r.disk_read_method || 'не указан'],
      ['Сеть, загрузка', number(r.network_download_mbps, ' Мбит/с')],
      ['SQLite, 1 поток', number(r.sqlite_single_tps, ' транз./с')],
      ['SQLite, 4 потока', number(r.sqlite_four_tps, ' транз./с')],
      ['SQLite, чтение p99', number(r.sqlite_read_p99_ms, ' мс')],
      ['Версия nmos', r.nmos_version || 'не указана']
    ]) data.append(metric(label, value));
    details.append(summary, data);
    if (Number.isInteger(r.source_issue) && r.source_issue > 0) {
      const source = element('a', `Issue #${r.source_issue}`, 'source-link');
      source.href = `https://github.com/rawfast-net/nmos/issues/${r.source_issue}`;
      details.append(source);
    }
    card.append(identity, cpu, metric('CPU steal под нагрузкой', number(r.steal_percent_load, '%')), metric('Сеть, загрузка', number(r.network_download_mbps, ' Мбит/с')), details);
    list.append(card);
  }
}
search.addEventListener('input', render);
sort.addEventListener('change', render);
let lastData = null;
let loading = false;
async function loadResults() {
  if (loading) return;
  loading = true;
  try {
    const response = await fetch(`./data/results.json?t=${Date.now()}`, { cache: 'no-store' });
    if (!response.ok) throw new Error('data unavailable');
    const data = await response.json();
    if (!Array.isArray(data)) throw new Error('invalid data');
    const serialized = JSON.stringify(data);
    if (serialized !== lastData) {
      lastData = serialized;
      results = data.filter(r => r && r.schema === 1 && typeof r.provider === 'string');
      render();
    }
  } catch {
    if (lastData === null) {
      count.textContent = '—';
      status.hidden = false;
      status.textContent = 'Не удалось загрузить результаты. Попробуйте обновить страницу.';
    }
  } finally {
    loading = false;
  }
}
loadResults();
setInterval(loadResults, 60000);
