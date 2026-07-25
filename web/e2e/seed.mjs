import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const orq =
  process.env.ORQ_BIN ||
  join(root, "target", "debug", process.platform === "win32" ? "orq.exe" : "orq");

if (!existsSync(orq)) {
  console.error(`orq binary missing: ${orq}\nRun: cargo build -p orq`);
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

run(["dash", "snapshot", "--json"]);

const statePath = join(__dirname, ".seed-state.json");
writeFileSync(
  statePath,
  JSON.stringify(
    {
      dataDir,
      orq,
      dashRoot: join(root, "web", "dashboard"),
    },
    null,
    2
  )
);
console.log("e2e seed ok ->", statePath);
