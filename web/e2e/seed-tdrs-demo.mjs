/**
 * Seed a td-rs–shaped `tdrs-loop` workspace for the static Pages demo.
 * Writes `.seed-tdrs-state.json` with dataDir / orq / dashRoot.
 *
 * Usage (from web/): node e2e/seed-tdrs-demo.mjs
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync, readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const fixtures = join(__dirname, "fixtures", "tdrs-demo");
const WS = "tdrs-loop";

const orq =
  process.env.ORQ_BIN ||
  join(root, "target", "release", process.platform === "win32" ? "porq.exe" : "porq");
const orqDebug = join(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "porq.exe" : "porq"
);
const orqBin = existsSync(orq) ? orq : orqDebug;

if (!existsSync(orqBin)) {
  console.error(`porq binary missing: ${orq}\nRun: cargo build --release -p orq`);
  process.exit(1);
}

const dataDir = mkdtempSync(join(tmpdir(), "orq-tdrs-demo-"));
process.env.ORQ_DATA_DIR = dataDir;
process.env.ORQ_WORKSPACE = WS;

function run(args, { allowFail = false } = {}) {
  const r = spawnSync(orqBin, ["-w", WS, ...args], {
    env: process.env,
    encoding: "utf8",
    shell: false,
  });
  if (!allowFail && r.status !== 0) {
    console.error(r.stdout || "");
    console.error(r.stderr || "");
    throw new Error(`porq failed (${r.status}): ${args.join(" ")}`);
  }
  return r;
}

function md(name) {
  const p = join(fixtures, name);
  if (!existsSync(p)) throw new Error(`missing fixture: ${p}`);
  return p;
}

console.log("tdrs-demo seed dataDir=", dataDir);

run(["init", "--json"]);

const tables = [
  {
    name: "meta",
    cols: ["marker:string:poi", "notes:string"],
  },
  {
    name: "roadmap",
    type: "roadmap",
    cols: ["status:string:poi", "kind:string", "order:number"],
  },
  {
    name: "gate-locks",
    cols: ["holder:string:poi", "gate:string", "note:string"],
  },
  {
    name: "roadmap-proposals",
    cols: ["state:string:poi", "gate:string", "proposer:string", "body:string"],
  },
  {
    name: "reports",
    cols: ["state:string:poi", "gate:string", "unit:string", "body:string"],
  },
  {
    name: "reviews",
    cols: ["state:string:poi", "task_id:string", "verdict:string", "body:string"],
  },
  {
    name: "sanity",
    cols: ["state:string:poi", "proposal_key:string", "verdict:string", "body:string"],
  },
  {
    name: "drift-reports",
    cols: ["state:string:poi", "subject:string", "severity:string", "body:string"],
  },
  {
    name: "computer",
    cols: [
      "purpose:string:poi",
      "holder_kind:string",
      "session:string",
      "yield_requested:bool",
      "yield_by:string",
      "note:string",
    ],
  },
];

for (const t of tables) {
  const args = ["poi", "table", "create", t.name, "--json"];
  if (t.type) args.splice(4, 0, "--table-type", t.type);
  for (const c of t.cols) {
    args.push("--cols", c);
  }
  run(args);
}

run(["poi", "set", "meta", "schema", "1", "--state", "v1", "--json"]);
run([
  "poi",
  "set",
  "meta",
  "schema-doc",
  JSON.stringify("docs/usecases/td-rs-autonomous-loop.md"),
  "--state",
  "active",
  "--json",
]);

run([
  "poi",
  "set",
  "roadmap",
  "_meta",
  JSON.stringify({
    active: null,
    stopState: "program-stop",
    note: "Active: none — showcase HANDOFF",
  }),
  "--state",
  "closed",
  "--col",
  "status=closed",
  "--col",
  "kind=meta",
  "--col",
  "order=0",
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

run([
  "poi",
  "set",
  "reviews",
  "exec-demo-scratch",
  JSON.stringify({
    verdict: "reject",
    task_id: "exec-demo-scratch",
    body: "Forced red — land veto path for static demo",
  }),
  "--state",
  "blocked",
  "--col",
  "state=blocked",
  "--col",
  "task_id=exec-demo-scratch",
  "--col",
  "verdict=reject",
  "--col",
  "body=Forced red review for demo",
  "--json",
]);

// Triggers (placeholder actions — static demo never fires them live)
run([
  "trigger",
  "add",
  "review-on-exec",
  "--on",
  "task.done",
  "--where-cond",
  "name^=exec-",
  "--do-action",
  "spawn:cmd /c exit 0",
  "--max-fires-per-hour",
  "30",
  "--json",
]);
run([
  "trigger",
  "add",
  "veto-land",
  "--on",
  "task.pre-exec",
  "--where-cond",
  "name^=land-",
  "--blocking",
  "--do-action",
  "spawn:cmd /c exit 1",
  "--max-fires-per-hour",
  "60",
  "--json",
]);
run([
  "trigger",
  "add",
  "sanity-on-proposal",
  "--on",
  "poi.changed",
  "--where-cond",
  "table==roadmap-proposals && state==proposed",
  "--do-action",
  "spawn:cmd /c exit 0",
  "--max-fires-per-hour",
  "20",
  "--json",
]);

// Claimed exec unit (Details noise)
run([
  "run",
  "--sync",
  "--name",
  "exec-demo-scratch",
  "--claim",
  "demo/**",
  "--json",
  "--",
  "echo",
  "exec-demo-scratch ok",
]);

// land-* should be blocked by veto-land (blocking pre-exec)
run(
  [
    "run",
    "--sync",
    "--name",
    "land-demo-scratch",
    "--json",
    "--",
    "echo",
    "should-not-run",
  ],
  { allowFail: true }
);

const canvases = [
  { key: "loop-health", file: "loop-health.md", title: "Loop Health", order: "1" },
  { key: "loop-roadmap", file: "loop-roadmap.md", title: "Loop Roadmap", order: "2" },
  { key: "loop-preflight", file: "loop-preflight.md", title: "Loop Preflight", order: "3" },
  { key: "loop-checks", file: "loop-checks.md", title: "Loop Checks", order: "4" },
  { key: "loop-review", file: "loop-review.md", title: "Loop Review", order: "5" },
  { key: "computer-focus", file: "computer-focus.md", title: "Computer focus", order: "6" },
];

for (const c of canvases) {
  run([
    "canvas",
    "set",
    c.key,
    "--title",
    c.title,
    "--md",
    md(c.file),
    "--order",
    c.order,
    "--json",
  ]);
}

run(["dash", "snapshot", "--json"]);

const statePath = join(__dirname, ".seed-tdrs-state.json");
writeFileSync(
  statePath,
  JSON.stringify(
    {
      dataDir,
      orq: orqBin,
      dashRoot: join(root, "web", "dashboard"),
      workspace: WS,
      snapshot: join(dataDir, "dash", "data.json"),
    },
    null,
    2
  )
);
console.log("tdrs-demo seed ok ->", statePath);
console.log(
  "canvases:",
  canvases.map((c) => c.key).join(", "),
  "| body chars sample:",
  readFileSync(md("loop-health.md"), "utf8").length
);
