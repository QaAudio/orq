# Recipe: central-committer

Single-repo vibe-coded projects: serialize git commits to avoid conflicts.

## Pattern

1. POI `git/head` holds the write lease for the committer identity forever (renew TTL).
2. Agents enqueue rows in table `commit_requests` (ephemeral).
3. Service task `committer` loops: claim next pending request, run `git commit`, mark done.

```bash
orq poi table create commit_requests --cols message:string:poi --cols status:string
orq poi lock git head --holder committer --ttl 86400 --reason "central committer"
orq run --kind service --restart always --name committer --claim ".git/**" -- "…"
orq poi set commit_requests req-1 '{"message":"feat: x"}' --state pending --tier ephemeral
```

Workers never call `git commit` directly; they only enqueue.
