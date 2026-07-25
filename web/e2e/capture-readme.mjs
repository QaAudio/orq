/**
 * Capture README dashboard assets:
 * - docs/img/dashboard.png          (Canvases — feature showcase)
 * - docs/img/dashboard-details.png  (Details view)
 * - docs/img/dashboard.gif          (toggle between the two)
 * - docs/img/gallery/theme-*.png    (dark / light)
 * - docs/img/gallery/scale-*.png    (100% / 125% / 150% CSS zoom)
 *
 * Usage (from web/): node e2e/capture-readme.mjs
 */
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:net";
import {
  mkdirSync,
  readFileSync,
  existsSync,
  writeFileSync,
  unlinkSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const outDir = join(root, "docs", "img");
const galleryDir = join(outDir, "gallery");
const outCanvases = join(outDir, "dashboard.png");
const outDetails = join(outDir, "dashboard-details.png");
const outGif = join(outDir, "dashboard.gif");

/** Readable 12-col layout so mermaid + HTML fit at 1440×900. */
const SHOWCASE_LAYOUT = {
  v: 1,
  cols: 12,
  rowHeight: 40,
  items: {
    plan: { x: 0, y: 0, w: 7, h: 11 },
    status: { x: 7, y: 0, w: 5, h: 7 },
    probe: { x: 7, y: 7, w: 5, h: 4 },
    log: { x: 0, y: 11, w: 12, h: 5 },
  },
};

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

async function waitReady(page) {
  await page.waitForFunction(() => {
    const stamp = document.querySelector("#stamp")?.textContent ?? "";
    return stamp && !stamp.includes("connecting") && !stamp.includes("waiting");
  }, null, { timeout: 15_000 });
  await page.waitForSelector(".canvas-card", { timeout: 10_000 });
  await page.waitForSelector("#pulse-event", { timeout: 10_000 });
  // Mermaid lazy-renders into the plan card
  await page.waitForFunction(
    () =>
      document.querySelectorAll(
        '.canvas-card[data-key="plan"] .canvas-md pre.mermaid svg'
      ).length > 0,
    null,
    { timeout: 15_000 }
  );
  await page.waitForTimeout(400);
}

async function gotoCanvases(page, baseURL, theme) {
  await page.addInitScript(
    ({ t, layout }) => {
      try {
        localStorage.setItem("porq.dash.view", "canvases");
        localStorage.setItem("porq.dash.qa-theme", t);
        localStorage.setItem("porq.dash.layout.canvases", JSON.stringify(layout));
        localStorage.setItem("porq.dash.filter.state", "active");
      } catch {
        /* ignore */
      }
    },
    { t: theme, layout: SHOWCASE_LAYOUT }
  );
  await page.goto(`${baseURL}/?theme=${theme}`);
  await waitReady(page);
  await page.waitForFunction(
    (t) => document.documentElement.getAttribute("data-qa-theme") === t,
    theme,
    { timeout: 5_000 }
  );
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

// Feature-showcase canvases for the README shot
for (const key of ["plan", "render", "mystery", "mission", "status", "probe", "log"]) {
  runOrq(state.orq, state.dataDir, ["canvas", "rm", key, "--json"], { allowFail: true });
}

const planBody = [
  "# Loop plan",
  "",
  "**State:** ok",
  "",
  "**Next:** `porq status --json --limit 20`",
  "",
  "Updated 2026-07-25T18:00:00Z",
  "",
  "| lane | state | note |",
  "| --- | --- | --- |",
  "| alpha | pending | claim `src/**` |",
  "| beta | done | checks green |",
  "| review | blocked | human gate |",
  "",
  "## Minimap",
  "",
  "```mermaid",
  "flowchart LR",
  '  A["prev done"] --> B["current ACTIVE"]',
  '  B --> C["next"]',
  "```",
].join("\n");

runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "plan",
  "--title",
  "Loop plan",
  "--order",
  "1",
  "--body",
  planBody,
  "--json",
]);

// HTML canvas: authoring uses bridge var names (system-color fallbacks inside sandboxed srcdoc)
const htmlPath = join(state.dataDir, "status-card.html");
writeFileSync(
  htmlPath,
  `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<style>
  :root {
    color-scheme: dark light;
    --text: CanvasText;
    --muted: GrayText;
    --accent: LinkText;
    --bg: Canvas;
    --panel: Field;
    --border: GrayText;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    padding: 14px 16px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.45;
    background: var(--panel);
    color: var(--text);
  }
  h1 {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 700;
    color: var(--accent);
  }
  .muted { color: var(--muted); font-size: 11px; margin-bottom: 12px; }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 0;
    border-top: 1px solid var(--border);
  }
  .label { color: var(--muted); }
  .value { color: var(--text); font-weight: 600; }
  .gauge {
    margin-top: 12px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
  }
  .gauge strong { color: var(--accent); font-size: 18px; }
</style>
</head>
<body>
  <h1>status</h1>
  <div class="muted">workspace · default · theme bridge</div>
  <div class="row"><span class="label">tasks done</span><span class="value">4</span></div>
  <div class="row"><span class="label">MoA route</span><span class="value">sticky</span></div>
  <div class="row"><span class="label">canvases live</span><span class="value">4</span></div>
  <div class="gauge">affinity <strong>code.edit</strong> → stub1</div>
</body>
</html>`
);
runOrq(state.orq, state.dataDir, [
  "canvas",
  "set",
  "status",
  "--title",
  "Status",
  "--html",
  htmlPath,
  "--height",
  "220",
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
  [
    "# Changelog",
    "",
    "**State:** live",
    "",
    "1. **plan** markdown + mermaid minimap published",
    "2. **status** HTML card uses theme bridge CSS vars",
    "3. **probe** image attached via `canvas:` asset",
    "4. Shell chrome: SDK tabs, badges, theme picker — not inside canvas bodies",
  ].join("\n"),
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
mkdirSync(galleryDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.addInitScript((layout) => {
  try {
    localStorage.setItem("porq.dash.view", "canvases");
    localStorage.setItem("porq.dash.qa-theme", "dark");
    localStorage.setItem("porq.dash.layout.canvases", JSON.stringify(layout));
    localStorage.setItem("porq.dash.filter.state", "active");
  } catch {
    /* ignore */
  }
}, SHOWCASE_LAYOUT);
await page.goto(baseURL + "/");
await waitReady(page);
await page.waitForFunction(
  () => document.documentElement.getAttribute("data-qa-theme") === "dark",
  null,
  { timeout: 5_000 }
);
await page.screenshot({ path: outCanvases, fullPage: false });

await page.locator("#tab-details").click();
await page.waitForSelector("#view-details.active", { timeout: 5_000 });
await page.waitForSelector("#board", { timeout: 5_000 });
await page.waitForTimeout(400);
await page.screenshot({ path: outDetails, fullPage: false });

// Theme gallery (dark / light)
const themes = ["dark", "light"];
for (const theme of themes) {
  const themePage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await gotoCanvases(themePage, baseURL, theme);
  const dest = join(galleryDir, `theme-${theme}.png`);
  await themePage.screenshot({ path: dest, fullPage: false });
  console.log("wrote", dest);
  await themePage.close();
}

// UI scale gallery (CSS zoom) — same viewport, denser/looser chrome
const scales = [
  { label: "100", zoom: 1 },
  { label: "125", zoom: 1.25 },
  { label: "150", zoom: 1.5 },
];
for (const { label, zoom } of scales) {
  const scalePage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await gotoCanvases(scalePage, baseURL, "dark");
  await scalePage.evaluate((z) => {
    document.documentElement.style.zoom = String(z);
  }, zoom);
  await scalePage.waitForTimeout(350);
  const dest = join(galleryDir, `scale-${label}.png`);
  await scalePage.screenshot({ path: dest, fullPage: false });
  console.log("wrote", dest);
  await scalePage.close();
}

await browser.close();
proc.kill();

// Drop obsolete gallery / alias files from pre-SDK themes
for (const stale of [
  join(galleryDir, "theme-default.png"),
  join(galleryDir, "theme-dracula.png"),
  join(galleryDir, "theme-system.png"),
  join(outDir, "usecases", "porq-demo-dracula.png"),
]) {
  try {
    unlinkSync(stale);
    console.log("removed", stale);
  } catch {
    /* missing ok */
  }
}

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
console.log("gallery ->", galleryDir);
