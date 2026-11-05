# nmos

**NoMoreOverSell** — CLI для проверки, совпадает ли VDS с тем, что продали.

Снимает CPU, RAM, диск и сеть, сверяет с тарифом (`--claim-*`). Имеет смысл, если подозреваете оверселл, урезанные ядра, balloon RAM или подмену модели CPU.

Проект от [rawfast.net](https://rawfast.net).

## Установка

### Из исходников (на VDS)

```bash
git clone https://github.com/rawfast-net/nmos.git
cd nmos
bash install.sh
```

Нужны Rust (`rustup`) и C-toolchain. Бинарник ставится в `/usr/local/bin/nmos`.

### Сборка на Windows (Docker → Debian 12)

```powershell
powershell -ExecutionPolicy Bypass -File .\build-linux.ps1
scp dist\nmos root@HOST:/usr/local/bin/nmos
```

## Запуск

```bash
# быстрый скан
nmos

# сверка с тарифом
nmos \
  --claim-cores 4 \
  --claim-ram-gb 8 \
  --claim-disk-gb 80 \
  --claim-cpu "EPYC 7763" \
  --claim-net-mbps 1000 \
  -d 6

# только JSON
nmos --json --claim-cores 2 --claim-ram-gb 4

# без тяжёлых тестов
nmos --skip-stress --skip-speed --skip-disk
```

### Флаги

| Флаг | По умолчанию | Назначение |
|------|--------------|------------|
| `-d, --duration` | `4` | длительность стресс-фазы, сек |
| `--claim-cores` | — | сколько vCPU продали |
| `--claim-ram-gb` | — | RAM по тарифу, GB |
| `--claim-disk-gb` | — | диск по тарифу, GB |
| `--claim-cpu` | — | модель CPU из тарифа |
| `--claim-net-mbps` | — | канал по тарифу, Mbps |
| `--region` | `auto` | `auto` / `ru` / `intl` / `all` |
| `--speed-mb` | `100` | размер download-теста |
| `--disk-mb` | `256` | размер disk I/O теста |
| `--mem-mb` | `512` | размер проверки RAM |
| `--offline` | off | без внешних запросов |
| `--json` | off | машинный отчёт |

## Что проверяет

- модель CPU / флаги / CPUID против каталога
- steal, cgroup quota, PSI, throttle
- stress ST/MT scaling (признак оверселла)
- alloc RAM + проверка паттерна
- диск seq / random 4K + checksum после fsync
- внешний IP, latency, скорость (Selectel / intl)

Цифры приблизительные. На shared лучше гонять вне пика с `-d 5..10` и без нагрузки на машине.

## Лицензия

MIT
