# Recipe: queue-drain

Generalized td-rs scrape-queue: POI queue table + parallel drain + single-flight apply.

```bash
orq poi table create queue --cols item:string:poi --cols status:string
orq poi set queue op1 '{"item":"constant"}' --state pending
orq poi set queue op2 '{"item":"noise"}' --state pending
# parallel drain workers (claims keep apply serialized via apply lease)
orq run --name drain-a --claim "queue/apply" -- "echo drain"
# apply holder:
orq poi lock queue apply --holder applier --ttl 600 --reason single-flight
```

Statuses: `pending` → `done` | `error` | `blocked`. Resume = list where `state==pending`.
