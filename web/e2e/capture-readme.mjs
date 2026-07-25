/**
 * Capture README dashboard assets:
 * - docs/img/dashboard.png          (Canvases view)
 * - docs/img/dashboard-details.png  (Details view)
 * - docs/img/dashboard.gif          (toggle between the two)
 *
 * Usage (from web/): node e2e/capture-readme.mjs
 */
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:net";
import { mkdirSync, readFileSync, existsSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const outDir = join(root, "docs", "img");
const outCanvases = join(outDir, "dashboard.png");
const outDetails = join(outDir, "dashboard-details.png");
const outGif = join(outDir, "dashboard.gif");
const outDracula = join(outDir, "usecases", "porq-demo-dracula.png");

function freePort() {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const addr = s.address();
      if (!addr || typeof addr === "string") {
        s.close();
        reject(new Error("no port"));
        return;
      }
      const port = addr.port;
      s.close(() => resolve(port));
    });
  });
}

function runOrq(orq, dataDir, args, { allowFail = false } = {}) {
  const r = spawnSync(orq, args, {
    env: { ...process.env, ORQ_DATA_DIR: dataDir, ORQ_WORKSPACE: "default" },
    encoding: "utf8",
  });
  if (!allowFail && r.status !== 0) {
    console.error(r.stdout || "");
    console.error(r.stderr || "");
    throw new Error(`porq failed: ${args.join(" ")}`);
  }
  return r;
}

// Seed first (board/tasks + basic canvases)
const seed = spawnSync(process.execPath, [join(__dirname, "seed.mjs")], {
  encoding: "utf8",
  cwd: join(__dirname, ".."),
});
if (seed.status !== 0) {
  console.error(seed.stdout);
  console.error(seed.stderr);
  process.exit(seed.status ?? 1);
}

const statePath = join(__dirname, ".seed-state.json");
if (!existsSync(statePath)) {
  console.error("missing .seed-state.json after seed");
  process.exit(1);
}
const state = JSON.parse(readFileSync(statePath, "utf8"));

// Rich demo canvases for the README shot
runOrq(state.orq, state.dataDir, ["canvas", "rm", "plan", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "render", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "mystery", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "mission", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "status", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "probe", "--json"], { allowFail: true });
runOrq(state.orq, state.dataDir, ["canvas", "rm", "log", "--json"], { allowFail: true });

runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "mission",
  "--title",
  "Mission",
  "--order",
  "1",
  "--body",
  "## Ship the desk\n\n- lock paths with `--claim`\n- publish status canvases\n- keep Details for ops\n\nRun `porq dash serve` and watch the pulse.",
  "--json",
]);

const svgPath = join(state.dataDir, "status-card.html");
writeFileSync(
  svgPath,
  `<!DOCTYPE html><html><body style="margin:0;background:#141210;font-family:IBM Plex Mono,monospace;color:#f2ebe3">
<svg viewBox="0 0 420 220" width="100%" xmlns="http://www.w3.org/2000/svg">
  <rect width="420" height="220" rx="12" fill="#1c1916" stroke="#3a342c"/>
  <text x="24" y="40" fill="#e8a838" font-size="18" font-family="Syne,sans-serif" font-weight="700">status</text>
  <text x="24" y="68" fill="#9a9084" font-size="12">workspace · default</text>
  <circle cx="320" cy="110" r="54" fill="none" stroke="#2a2520" stroke-width="14"/>
  <circle cx="320" cy="110" r="54" fill="none" stroke="#3db8a0" stroke-width="14"
    stroke-dasharray="240 340" stroke-linecap="round" transform="rotate(-90 320 110)"/>
  <text x="320" y="116" text-anchor="middle" fill="#f2ebe3" font-size="22" font-weight="600">87%</text>
  <text x="24" y="120" fill="#6bbf7a" font-size="13">● 4 tasks done</text>
  <text x="24" y="148" fill="#e8a838" font-size="13">● MoA route sticky</text>
  <text x="24" y="176" fill="#6a9fbf" font-size="13">● 3 canvases live</text>
  <text x="24" y="204" fill="#9a9084" font-size="11">affinity code.edit → stub1</text>
</svg></body></html>`
);
runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "status",
  "--title",
  "Status",
  "--html",
  svgPath,
  "--height",
  "240",
  "--order",
  "2",
  "--json",
]);

const pngPath = join(state.dataDir, "probe.png");
writeFileSync(
  pngPath,
  Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAYAAACNMs+9AAAAFUlEQVR42mNk+M9Qz0AEYBxVSF+FABJADveWkH6oAAAAAElFTkSuQmCC",
    "base64"
  )
);
runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "probe",
  "--title",
  "Probe",
  "--image",
  pngPath,
  "--alt",
  "probe tile",
  "--order",
  "3",
  "--json",
]);

runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "log",
  "--title",
  "What just happened",
  "--span",
  "2",
  "--order",
  "4",
  "--body",
  "## Changelog\n\n1. **mission** canvas published\n2. SVG **status** gauge rendered (sandboxed html)\n3. **probe** image attached via `canvas:` asset\n4. Pulse strip shows live counts without leaving Canvases view",
  "--json",
]);

runOrq(state.orq, state.dataDir, ["dash", "snapshot", "--json"]);

const port = await freePort();
const baseURL = `http://127.0.0.1:${port}`;

const proc = spawn(
  state.orq,
  ["dash", "serve", "--port", String(port), "--root", state.dashRoot],
  {
    env: {
      ...process.env,
      ORQ_DATA_DIR: state.dataDir,
      ORQ_WORKSPACE: "default",
    },
    stdio: ["ignore", "pipe", "pipe"],
  }
);

await new Promise((resolve, reject) => {
  const t = setTimeout(() => reject(new Error("dash serve timeout")), 15_000);
  const onData = (buf) => {
    const s = buf.toString();
    if (s.includes("porq dash serve") || s.includes("orq dash serve")) {
      clearTimeout(t);
      resolve();
    }
  };
  proc.stdout?.on("data", onData);
  proc.stderr?.on("data", onData);
  proc.on("exit", (code) => {
    clearTimeout(t);
    reject(new Error(`dash serve exited early: ${code}`));
  });
});

mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.addInitScript(() => {
  try {
    localStorage.setItem("porq.dash.view", "canvases");
  } catch {
    /* ignore */
  }
});
await page.goto(baseURL + "/");
await page.waitForFunction(() => {
  const stamp = document.querySelector("#stamp")?.textContent ?? "";
  return stamp && !stamp.includes("connecting") && !stamp.includes("waiting");
}, null, { timeout: 15_000 });
await page.waitForSelector(".canvas-card", { timeout: 10_000 });
await page.waitForSelector("#pulse-event", { timeout: 10_000 });
await page.waitForTimeout(700);
await page.screenshot({ path: outCanvases, fullPage: false });

await page.locator("#tab-details").click();
await page.waitForSelector("#view-details.active", { timeout: 5_000 });
await page.waitForSelector("#board", { timeout: 5_000 });
await page.waitForTimeout(500);
await page.screenshot({ path: outDetails, fullPage: false });

// Dracula still for themes docs (hero stays on default)
await page.locator("#tab-canvases").click();
await page.waitForSelector("#view-canvases.active", { timeout: 5_000 });
await page.locator("#theme-select").selectOption("dracula");
await page.waitForFunction(
  () => document.documentElement.getAttribute("data-theme") === "dracula"
);
await page.waitForTimeout(400);
mkdirSync(dirname(outDracula), { recursive: true });
await page.screenshot({ path: outDracula, fullPage: false });

await browser.close();
proc.kill();

const gifPy = `
from PIL import Image
a = Image.open(r'''${outCanvases.replace(/\\/g, "/")}''').convert("P", palette=Image.ADAPTIVE, colors=128)
b = Image.open(r'''${outDetails.replace(/\\/g, "/")}''').convert("P", palette=Image.ADAPTIVE, colors=128)
a.save(
    r'''${outGif.replace(/\\/g, "/")}''',
    save_all=True,
    append_images=[b],
    duration=1800,
    loop=0,
    optimize=True,
)
print("gif ok")
`;
const gif = spawnSync("python", ["-c", gifPy], { encoding: "utf8" });
if (gif.status !== 0) {
  console.error(gif.stdout || "");
  console.error(gif.stderr || "");
  process.exit(gif.status ?? 1);
}

console.log("wrote", outCanvases);
console.log("wrote", outDetails);
console.log("wrote", outGif);
console.log("wrote", outDracula);
