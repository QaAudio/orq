import { spawnSync, spawn } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const orq =
  process.env.ORQ_BIN ||
  join(root, "target", "debug", process.platform === "win32" ? "porq.exe" : "porq");

if (!existsSync(orq)) {
  console.error(`porq binary missing: ${orq}\nRun: cargo build -p orq`);
  process.exit(1);
}

const dataDir = mkdtempSync(join(tmpdir(), "orq-e2e-"));
process.env.ORQ_DATA_DIR = dataDir;
process.env.ORQ_WORKSPACE = "default";

function run(args, { allowFail = false } = {}) {
  const r = spawnSync(orq, args, {
    env: process.env,
    encoding: "utf8",
    shell: false,
  });
  if (!allowFail && r.status !== 0) {
    console.error(r.stdout || "");
    console.error(r.stderr || "");
    throw new Error(`orq failed (${r.status}): ${args.join(" ")}`);
  }
  return r;
}

console.log("e2e seed dataDir=", dataDir);

run(["init", "--json"]);
run([
  "poi",
  "table",
  "create",
  "board",
  "--cols",
  "owner:string",
  "--json",
]);
run([
  "poi",
  "set",
  "board",
  "alpha",
  JSON.stringify({ note: "pending lane" }),
  "--state",
  "pending",
  "--col",
  "owner=alpha",
  "--json",
]);
run([
  "poi",
  "set",
  "board",
  "beta",
  JSON.stringify({ note: "done lane" }),
  "--state",
  "done",
  "--col",
  "owner=beta",
  "--json",
]);
run([
  "poi",
  "set",
  "board",
  "review",
  JSON.stringify({ note: "blocked lane" }),
  "--state",
  "blocked",
  "--col",
  "owner=review",
  "--json",
]);
run([
  "poi",
  "set",
  "board",
  "oldlane",
  JSON.stringify({ note: "archived lane" }),
  "--state",
  "archived",
  "--col",
  "owner=archive",
  "--json",
]);

run([
  "model",
  "add",
  "stub1",
  "--cli",
  "echo STUB1:{cmd}",
  "--capability",
  "code",
  "--json",
]);
run([
  "model",
  "add",
  "stub2",
  "--cli",
  "echo STUB2:{cmd}",
  "--capability",
  "code",
  "--json",
]);
run([
  "model",
  "add",
  "agg",
  "--cli",
  "echo AGG:{cmd}",
  "--capability",
  "code",
  "--json",
]);
run(["affinity", "set", "code.edit", "stub1", "--score", "0.9", "--json"]);
run(["affinity", "set", "code.edit", "stub2", "--score", "0.5", "--json"]);

run([
  "run",
  "--sync",
  "--class",
  "code.edit",
  "--strategy",
  "single",
  "--seed",
  "1",
  "--name",
  "e2e-task",
  "--json",
  "--",
  "echo",
  "E2E",
]);

// Detached long-lived task for Running tasks + Board claim highlight (key "alpha")
{
  const args = [
    "run",
    "--claim",
    "alpha",
    "--name",
    "e2e-running",
    "--json",
    "--",
    ...(process.platform === "win32"
      ? ["cmd", "/c", "ping", "-n", "300", "127.0.0.1"]
      : ["sleep", "300"]),
  ];
  const child = spawn(orq, args, {
    env: process.env,
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  child.unref();
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    const st = run(["status", "--json", "--limit", "40"], { allowFail: true });
    const out = `${st.stdout || ""}${st.stderr || ""}`;
    if (/e2e-running/i.test(out) && /running|starting/i.test(out)) break;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 400);
  }
}

run([
  "run",
  "--sync",
  "--class",
  "code.edit",
  "--strategy",
  "moa",
  "--moa-k",
  "2",
  "--moa-aggregator",
  "agg",
  "--seed",
  "2",
  "--name",
  "e2e-moa",
  "--json",
  "--",
  "echo",
  "MOA",
]);

// Canvases: markdown + image (+ unknown-kind fallback via raw poi set)
run([
  "canvas",
  "set",
  "plan",
  "--title",
  "E2E Plan",
  "--body",
  [
    "## E2E Plan",
    "",
    "**State:** ok | **Generated:** 2026-07-25T12:00:00Z",
    "",
    "- seed board",
    "- publish canvas",
    "- assert render",
    "",
    "```mermaid",
    "flowchart LR",
    '  A["start"] --> B["done"]',
    "```",
  ].join("\n"),
  "--order",
  "1",
  "--json",
]);

const pngPath = join(dataDir, "e2e-dot.png");
// 1x1 PNG
writeFileSync(
  pngPath,
  Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
    "base64"
  )
);
run([
  "canvas",
  "set",
  "render",
  "--title",
  "E2E Render",
  "--image",
  pngPath,
  "--alt",
  "e2e pixel",
  "--order",
  "2",
  "--json",
]);

run([
  "poi",
  "table",
  "create",
  "canvas",
  "--json",
], { allowFail: true });
run([
  "poi",
  "set",
  "canvas",
  "mystery",
  JSON.stringify({ v: 1, kind: "vega-lite", title: "Future chart", spec: { mark: "bar" } }),
  "--state",
  "live",
  "--col",
  "order=3",
  "--json",
]);

run([
  "poi",
  "table",
  "create",
  "computer",
  "--cols",
  "purpose:string:poi",
  "--cols",
  "holder_kind:string",
  "--cols",
  "session:string",
  "--cols",
  "yield_requested:bool",
  "--cols",
  "yield_by:string",
  "--cols",
  "note:string",
  "--json",
]);
run([
  "poi",
  "set",
  "computer",
  "focus",
  JSON.stringify({
    v: 1,
    purpose: "",
    holder_kind: "",
    session: "",
    yield_requested: false,
    yield_by: null,
    note: "",
  }),
  "--state",
  "idle",
  "--json",
]);

run(["dash", "snapshot", "--json"]);

const statePath = join(__dirname, ".seed-state.json");
writeFileSync(
  statePath,
  JSON.stringify(
    {
      dataDir,
      orq,
      dashRoot: join(root, "web", "dashboard", "dist"),
    },
    null,
    2
  )
);
console.log("e2e seed ok ->", statePath);
