# Recipe: queue-drain

Generalized td-rs scrape-queue: POI queue table + parallel drain + single-flight apply.

```bash
porq poi table create queue --cols item:string:poi --cols status:string
porq poi set queue op1 '{"item":"constant"}' --state pending
porq poi set queue op2 '{"item":"noise"}' --state pending
# parallel drain workers (claims keep apply serialized via apply lease)
porq run --name drain-a --claim "queue/apply" -- "echo drain"
# apply holder:
porq poi lock queue apply --holder applier --ttl 600 --reason single-flight
```

Statuses: `pending` → `done` | `error` | `blocked`. Resume = list where `state==pending`.
