# Qualification comparison tables

All latency entries are medians of per-trial percentiles, not percentiles pooled across requests.
Ranges show observed per-trial goodput; native cells generally have three repetitions.

## Controlled virtual-time matrix

| Scenario | CUBIC Mbps | BBRv3 Mbps | Change | CUBIC p95 ms | BBRv3 p95 ms | BBRv3 Mbps range |
|---|---:|---:|---:|---:|---:|---:|
| clean | 18.84 | 18.39 | -2.4% | 120.0 | 46.0 | 18.39–18.39 |
| wan | 18.84 | 18.27 | -3.0% | 178.0 | 142.5 | 18.26–18.28 |
| loss-0.1pct | 4.66 | 17.74 | +280.5% | 96.0 | 142.0 | 17.16–17.93 |
| loss-1pct | 1.20 | 16.87 | +1306.4% | 119.5 | 148.0 | 16.19–17.20 |
| deep-queue | 19.03 | 18.31 | -3.8% | 541.0 | 67.0 | 18.30–18.32 |
| shallow-queue | 17.42 | 17.51 | +0.5% | 91.0 | 91.0 | 17.33–17.61 |
| recovery | 4.54 | 11.79 | +159.8% | 103.5 | 183.5 | 11.43–12.62 |
| rpc-only | 0.00 | 0.00 | — | 82.0 | 82.0 | 0.00–0.00 |

## Native controlled comparisons

| Host/path | Scenario | CUBIC Mbps | BBRv3 Mbps | CUBIC p95 ms | BBRv3 p95 ms |
|---|---|---:|---:|---:|---:|
| hetzner | clean | 18.63 | 18.29 | 117.6 | 46.9 |
| hetzner | wan | 18.63 | 18.13 | 177.0 | 149.7 |
| hetzner | loss-1pct | 1.83 | 16.61 | 112.4 | 105.0 |
| hetzner | deep-queue | 19.02 | 18.26 | 535.1 | 86.1 |
| hetzner | shallow-queue | 17.50 | 17.05 | 90.8 | 89.5 |
| hetzner | recovery | 12.65 | 12.33 | 174.3 | 181.3 |
| hetzner | rpc-only | 0.00 | 0.00 | 82.4 | 82.6 |
| hetzner | wan-no-gso | 18.62 | 18.34 | 175.3 | 149.4 |
| linbox | clean | 18.63 | 18.28 | 117.4 | 49.5 |
| linbox | wan | 18.63 | 18.37 | 177.1 | 151.6 |
| linbox | loss-1pct | 1.51 | 16.89 | 111.8 | 140.1 |
| linbox | deep-queue | 19.03 | 18.41 | 536.5 | 82.9 |
| linbox | shallow-queue | 16.84 | 17.60 | 90.1 | 89.4 |
| linbox | recovery | 9.02 | 12.33 | 158.6 | 181.7 |
| linbox | rpc-only | 0.00 | 0.00 | 82.2 | 82.7 |
| linbox | wan-no-gso | 18.62 | 18.32 | 175.3 | 143.3 |
| ovh | clean | 18.63 | 18.28 | 117.4 | 49.6 |
| ovh | wan | 18.63 | 18.38 | 177.4 | 154.7 |
| ovh | loss-1pct | 1.93 | 17.07 | 132.2 | 142.5 |
| ovh | deep-queue | 19.01 | 18.25 | 537.4 | 82.9 |
| ovh | shallow-queue | 17.78 | 16.83 | 91.2 | 89.5 |
| ovh | recovery | 12.69 | 13.10 | 172.9 | 181.0 |
| ovh | rpc-only | 0.00 | 0.00 | 82.2 | 82.7 |
| ovh | wan-no-gso | 18.62 | 18.34 | 175.8 | 140.9 |

## Native supplementary comparisons

| Host/path | Scenario | CUBIC Mbps | BBRv3 Mbps | CUBIC p95 ms | BBRv3 p95 ms |
|---|---|---:|---:|---:|---:|
| ovh | ecn-aqm | 17.65 | 17.14 | 91.3 | 148.3 |
| ovh | jitter-reorder | 11.96 | 9.33 | 155.6 | 170.3 |
| ovh | high-bdp | no completed runs | 91.79 | no completed runs | 147.7 |

## Native paths comparisons

| Host/path | Scenario | CUBIC Mbps | BBRv3 Mbps | CUBIC p95 ms | BBRv3 p95 ms |
|---|---|---:|---:|---:|---:|
| lan->mac | physical-lan | no completed runs | 78.54 | no completed runs | 26.4 |
| ovh->lan | ovh-to-lan | no completed runs | 128.89 | no completed runs | 82.7 |
| hetzner->ovh | hetzner-to-ovh | 929.81 | 915.26 | 25.2 | 24.9 |
| hetzner->ovh | hetzner-to-ovh-no-gso | 929.70 | 905.05 | 25.0 | 24.6 |
| lan->mac | physical-lan-default-flow | 78.88 | 76.15 | 152.7 | 26.0 |
| ovh->hetzner | ovh-to-hetzner-default-flow | 402.16 | 394.88 | 25.3 | 25.2 |

## Failed native connections

Failures are retained separately and are not included in completed-run throughput medians.

| Host/path | Scenario | Controller | Failed connections |
|---|---|---|---:|
| emrul-010 | high-bdp | cubic | 3 |
| lan->mac | physical-lan | cubic | 3 |
| ovh->hetzner | ovh-to-hetzner | bbr3 | 3 |
| ovh->hetzner | ovh-to-hetzner | cubic | 3 |
| ovh->lan | ovh-to-lan | cubic | 3 |
