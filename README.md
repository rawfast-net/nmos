# nmos

**NoMoreOverSell** — CLI to check whether a VDS matches what was sold.

Measures CPU, RAM, disk, and network, then compares results to plan claims (`--claim-*`). Useful when you suspect oversell, underclocked cores, ballooned RAM, or a fake CPU model.

Provided by [rawfast.net](https://rawfast.net).

## Install

### From source (on the VDS)

```bash
git clone https://github.com/rawfast-net/nmos.git
cd nmos
bash install.sh
```

Needs Rust (`rustup`) and a C toolchain. Binary goes to `/usr/local/bin/nmos`.

### Cross-build on Windows (Docker → Debian 12)

```powershell
powershell -ExecutionPolicy Bypass -File .\build-linux.ps1
scp dist\nmos root@HOST:/usr/local/bin/nmos
```

## Usage

```bash
# quick scan
nmos

# compare against the plan
nmos \
  --claim-cores 4 \
  --claim-ram-gb 8 \
  --claim-disk-gb 80 \
  --claim-cpu "EPYC 7763" \
  --claim-net-mbps 1000 \
  -d 6

# JSON only
nmos --json --claim-cores 2 --claim-ram-gb 4

# skip heavy tests
nmos --skip-stress --skip-speed --skip-disk
```

### Flags

| Flag | Default | Meaning |
|------|---------|---------|
| `-d, --duration` | `4` | stress phase length, seconds |
| `--claim-cores` | — | sold vCPU count |
| `--claim-ram-gb` | — | sold RAM, GB |
| `--claim-disk-gb` | — | sold disk, GB |
| `--claim-cpu` | — | sold CPU model string |
| `--claim-net-mbps` | — | sold bandwidth, Mbps |
| `--region` | `auto` | `auto` / `ru` / `intl` / `all` |
| `--speed-mb` | `100` | download test size |
| `--disk-mb` | `256` | disk I/O test size |
| `--mem-mb` | `512` | RAM alloc/verify size |
| `--offline` | off | no outbound requests |
| `--json` | off | machine-readable report |

## What it checks

- CPU model / flags / CPUID vs catalog
- Steal time, cgroup quota, PSI, throttle
- Stress ST/MT scaling (oversell signal)
- RAM alloc + pattern verify
- Disk seq / random 4K + checksum after fsync
- Public IP, latency, download speed (Selectel / intl)

Scores are approximate. On shared hosts, re-run off-peak with `-d 5..10` and idle load.

## License

MIT
