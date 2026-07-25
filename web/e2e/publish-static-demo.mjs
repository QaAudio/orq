/**
 * Publish static GitHub Pages demo:
 * - seed tdrs-loop narrative
 * - copy built web/dashboard/dist → docs/demo/
 * - write frozen data.json with static_demo: true
 * - write docs/index.html → redirect to demo/
 *
 * Usage (from web/): npm run publish:demo
 * Requires: cargo build --release -p orq (or ORQ_BIN)
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
  cpSync,
  readdirSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const dashSrc = join(root, "web", "dashboard", "dist");
const demoOut = join(root, "docs", "demo");
const docsIndex = join(root, "docs", "index.html");

if (!existsSync(join(dashSrc, "index.html"))) {
  console.error("missing built dash at", dashSrc, "— run npm run build first");
  process.exit(1);
}

const seed = spawnSync(process.execPath, [join(__dirname, "seed-tdrs-demo.mjs")], {
  encoding: "utf8",
  cwd: join(__dirname, ".."),
  env: process.env,
});
if (seed.status !== 0) {
  console.error(seed.stdout || "");
  console.error(seed.stderr || "");
  process.exit(seed.status ?? 1);
}
process.stdout.write(seed.stdout || "");

const statePath = join(__dirname, ".seed-tdrs-state.json");
if (!existsSync(statePath)) {
  console.error("missing .seed-tdrs-state.json after seed");
  process.exit(1);
}
const state = JSON.parse(readFileSync(statePath, "utf8"));
const snapPath = state.snapshot || join(state.dataDir, "dash", "data.json");
if (!existsSync(snapPath)) {
  console.error("missing snapshot:", snapPath);
  process.exit(1);
}

const snap = JSON.parse(readFileSync(snapPath, "utf8"));
snap.static_demo = true;
snap.demo = {
  kind: "static",
  narrative: "tdrs-loop",
  note: "Observe-only GitHub Pages demo — real dashboard UI, frozen snapshot.",
};

if (existsSync(demoOut)) {
  rmSync(demoOut, { recursive: true, force: true });
}
mkdirSync(demoOut, { recursive: true });
cpSync(dashSrc, demoOut, { recursive: true });

writeFileSync(join(demoOut, "data.json"), JSON.stringify(snap, null, 2) + "\n");

// Optional canvas asset dir (markdown demos usually inline; copy if present)
const canvasSrc = join(state.dataDir, "canvas");
if (existsSync(canvasSrc) && readdirSync(canvasSrc).length > 0) {
  cpSync(canvasSrc, join(demoOut, "canvas"), { recursive: true });
}

writeFileSync(
  docsIndex,
  `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta http-equiv="refresh" content="0; url=demo/" />
  <title>porq — redirect</title>
  <link rel="canonical" href="demo/" />
  <script>location.replace("demo/");</script>
</head>
<body>
  <p><a href="demo/">porq live demo</a></p>
</body>
</html>
`
);

console.log("published", demoOut);
console.log("redirect", docsIndex);
console.log(
  "canvases:",
  Array.isArray(snap.canvases) ? snap.canvases.map((c) => c.key).join(", ") : "(none)"
);
