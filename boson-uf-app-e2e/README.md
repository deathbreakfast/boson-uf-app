# boson-uf-app-e2e

Leptos lab host + Playwright for [`boson-app`](../boson-app/) `BosonRoutes`.

Mounts the same pages a product host would under `/boson`, with lab-only mem
Valence, session injection, and an in-process MemQueue Boson coordinator.
**Do not copy this boot into a production host.**

## Run

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-boson-uf-app
# From the boson-uf-app workspace root.
cargo leptos end-to-end --project boson-uf-app-e2e
```

Do not interrupt the end-to-end run. It stops on its own when Playwright finishes.

Site: `http://127.0.0.1:3170` · seed: `POST /api/test/seed-data`

## Scenarios

| ID | Asserts |
|----|---------|
| `pw-boson-auth-gate-*` | Anon gated; admin sees dashboard |
| `pw-boson-dashboard-*` | KPI / seeded task visible |
| `pw-boson-tasks-*` | List→detail; unknown task empty |
| `pw-boson-jobs-*` | `/boson/queue` redirects to Jobs on Active; Cancel asks, then names the job; non-admin denied; tabs don't reload the page |
| `pw-boson-failed-jobs-*` | Retry two selected failed jobs into Active; Drop removes one and its run stays in Runs |
| `pw-boson-task-config-*` | Unverified blocked; admin save; host pool provider rows offered and a picked pool persists |
| `pw-boson-runs-*` | List→detail; status, task and time filters in the URL; search by an id's last characters; total; Clear filters; unknown run |
