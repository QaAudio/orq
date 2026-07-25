/**
 * Capture a README screenshot of the seeded dashboard.
 * Usage (from web/): node e2e/capture-readme.mjs
 */
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:net";
import { mkdirSync, readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..");
const outDir = join(root, "docs", "img");
const outPath = join(outDir, "dashboard.png");

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

// Seed first
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
    if (buf.toString().includes("orq dash serve")) {
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
await page.goto(baseURL + "/");
await page.waitForFunction(() => {
  const stamp = document.querySelector("#stamp")?.textContent ?? "";
  return stamp && !stamp.includes("connecting") && !stamp.includes("waiting");
}, null, { timeout: 15_000 });
await page.waitForSelector("#events .event-row", { timeout: 10_000 });
// let fonts settle
await page.waitForTimeout(800);
await page.screenshot({ path: outPath, fullPage: false });
await browser.close();
proc.kill();

console.log("wrote", outPath);
