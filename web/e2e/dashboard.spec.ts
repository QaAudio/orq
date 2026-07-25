import { test, expect } from "@playwright/test";
import { spawn, ChildProcess } from "node:child_process";
import { createServer } from "node:net";
import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

async function freePort(): Promise<number> {
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

test.describe("orq dashboard", () => {
  let proc: ChildProcess | undefined;
  let baseURL = "";

  test.beforeAll(async () => {
    const statePath = join(__dirname, ".seed-state.json");
    if (!existsSync(statePath)) {
      throw new Error("missing e2e/.seed-state.json — run npm run test:e2e (seed first)");
    }
    const state = JSON.parse(readFileSync(statePath, "utf8")) as {
      dataDir: string;
      orq: string;
      dashRoot: string;
    };
    const port = await freePort();
    baseURL = `http://127.0.0.1:${port}`;
    proc = spawn(
      state.orq,
      [
        "dash",
        "serve",
        "--port",
        String(port),
        "--root",
        state.dashRoot,
      ],
      {
        env: {
          ...process.env,
          ORQ_DATA_DIR: state.dataDir,
          ORQ_WORKSPACE: "default",
        },
        stdio: ["ignore", "pipe", "pipe"],
      }
    );
    // wait until server announces
    await new Promise<void>((resolve, reject) => {
      const t = setTimeout(() => reject(new Error("dash serve timeout")), 15_000);
      const onData = (buf: Buffer) => {
        const s = buf.toString();
        if (s.includes("orq dash serve")) {
          clearTimeout(t);
          resolve();
        }
      };
      proc!.stdout?.on("data", onData);
      proc!.stderr?.on("data", onData);
      proc!.on("exit", (code) => {
        clearTimeout(t);
        reject(new Error(`dash serve exited early: ${code}`));
      });
    });
  });

  test.afterAll(async () => {
    if (proc && !proc.killed) {
      proc.kill();
    }
  });

  test("renders seeded board, tasks, and stamp", async ({ page }) => {
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", {
      timeout: 10_000,
    });
    await expect(page.locator("#stamp")).not.toHaveText("waiting for monitor…");
    await expect(page.locator("#stamp")).not.toHaveText(/^$/);
    await expect(page.locator("#stamp")).not.toHaveText("error");

    await expect(page.locator("#board")).toContainText("alpha");
    await expect(page.locator("#board")).toContainText("beta");
    await expect(page.locator("#tasks")).toContainText("e2e-task");
    await expect(page.locator("#error")).not.toHaveClass(/visible/);
    await expect(page.locator(".pill").first()).toBeVisible();
    await expect(page.locator("#events .event-row").first()).toBeVisible();

    // Canvas protocol
    await expect(page.locator(".canvas-card")).toHaveCount(3);
    await expect(page.locator('.canvas-card[data-key="plan"]')).toContainText("E2E Plan");
    await expect(page.locator('.canvas-card[data-key="plan"] .canvas-md h2')).toContainText(
      "E2E Plan"
    );
    const img = page.locator('.canvas-card[data-key="render"] img');
    await expect(img).toBeVisible();
    await expect
      .poll(async () => img.evaluate((el) => (el as HTMLImageElement).naturalWidth))
      .toBeGreaterThan(0);
    await expect(page.locator('.canvas-card[data-kind="vega-lite"] .canvas-fallback')).toContainText(
      "vega-lite"
    );
  });
});
