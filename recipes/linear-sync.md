# Recipe: linear-sync

Bidirectional living roadmap without embedding a Linear client in porq core.

## Pattern

1. POI table `roadmap` with columns `external_id`, `etag`, `title`, `status` (mixed plain + POI).
2. Service task `linear-poller` periodically fetches remote issues and applies:

```bash
porq poi set roadmap ISSUE-1 '{"title":"Ship orq"}' --col external_id=lin_abc --col etag=v3 --if-version 2
```

CAS (`--if-version`) prevents clobbering local agent edits.

3. Trigger on local `poi.changed` where `key` matches roadmap items spawns a push oneshot that POSTs to Linear (implemented by your script, not orq).

4. Budgets: `porq workspace budget --max-spawns-per-hour 30`

## Demo (local stand-in)

```bash
porq poi table create roadmap --cols title:string:poi --cols external_id:string --cols etag:string
porq poi set roadmap item-1 '{"title":"first"}' --col external_id=ext1 --col etag=1
porq poi set roadmap item-1 '{"title":"first-edited"}' --if-version 1 --col etag=2
# conflict:
porq poi set roadmap item-1 '{"title":"stale"}' --if-version 1 || true
```
