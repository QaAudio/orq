# From prompt to verified change

**State:** live

**Next:** `porq status --json --limit 20`

**Generated:** 2026-07-25T20:00:00Z

```mermaid
flowchart LR
  Brief["Brief"] --> Route{"Route work"}
  Route --> AgentA["Agent A"]
  Route --> AgentB["Agent B"]
  AgentA --> Claims["Path claims"]
  AgentB --> Claims
  Claims --> Checks["Parallel checks"]
  Checks --> Gate{"Review gate"}
  Gate -->|pass| Ship["Ship"]
  Gate -->|veto| Repair["Repair"]
  Repair --> Checks
```

The diagram is rendered client-side from a fenced Mermaid block. It inherits
the dashboard theme and runs with Mermaid's strict security mode.
