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

test.describe("porq dashboard", () => {
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
    if (!existsSync(join(state.dashRoot, "index.html"))) {
      throw new Error("missing built dash at " + state.dashRoot + " — run npm run build");
    }
    const port = await freePort();
    baseURL = `http://127.0.0.1:${port}`;
    proc = spawn(
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
    await new Promise<void>((resolve, reject) => {
      const t = setTimeout(() => reject(new Error("dash serve timeout")), 15_000);
      const onData = (buf: Buffer) => {
        const s = buf.toString();
        if (s.includes("porq dash serve") || s.includes("orq dash serve")) {
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

  test("canvas-first view + details + archived filter", async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.removeItem("porq.dash.view");
        localStorage.removeItem("porq.dash.filter.state");
      } catch {
        /* ignore */
      }
    });
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", {
      timeout: 10_000,
    });
    await expect(page.locator("#stamp")).not.toHaveText("error");
    await expect(page.locator(".brand-mark")).toHaveText("porq");

    await expect(page.locator("#view-canvases")).toHaveClass(/active/);
    await expect(page.locator("#view-details")).not.toHaveClass(/active/);
    await expect(page.locator(".canvas-card")).toHaveCount(3);
    await expect(page.locator('.canvas-card[data-key="plan"] .canvas-md h2')).toContainText(
      "E2E Plan"
    );
    await expect(
      page.locator('.canvas-card[data-key="plan"] .canvas-sub .qa-badge')
    ).toHaveText("markdown");
    await expect
      .poll(
        async () =>
          page.locator('.canvas-card[data-key="plan"] .canvas-md pre.mermaid svg').count(),
        { timeout: 15_000 }
      )
      .toBeGreaterThan(0);
    // Body ISO (Generated:) upgraded to TimeBadge — not left as raw T12:00:00Z text alone
    const bodyTime = page.locator(
      '.canvas-card[data-key="plan"] .canvas-md .time-badge[data-time-id="md:plan:0"]'
    );
    await expect(bodyTime).toBeVisible({ timeout: 10_000 });
    await expect(bodyTime).toHaveAttribute("title", "2026-07-25T12:00:00Z");
    await expect(bodyTime).not.toHaveText(/T12:00:00Z/);
    const img = page.locator('.canvas-card[data-key="render"] img');
    await expect(img).toBeVisible();
    await expect
      .poll(async () => img.evaluate((el) => (el as HTMLImageElement).naturalWidth))
      .toBeGreaterThan(0);
    await expect(page.locator('.canvas-card[data-kind="vega-lite"] .canvas-fallback')).toContainText(
      "vega-lite"
    );

    await expect(page.locator("#pulse-tasks")).not.toHaveText("0");
    await expect(page.locator("#pulse-pois")).not.toHaveText("0");
    await expect(page.locator("#pulse-event")).not.toHaveText("waiting for pulse…");

    await page.locator("#tab-details").click();
    await expect(page.locator("#view-details")).toHaveClass(/active/);
    await expect(page.locator("#board")).toContainText("alpha");
    await expect(page.locator("#board")).toContainText("beta");
    await expect(page.locator("#board")).not.toContainText("oldlane");
    await expect(page.locator("#board .value-details")).toHaveCount(3);
    await page.locator("#board .value-details summary").first().click();
    await expect(page.locator("#board .value-details[open] .json-pretty").first()).toContainText(
      "pending lane"
    );

    await page.locator('[data-height-panel="board"] .qa-segmented button').filter({ hasText: "All" }).click();
    await expect(page.locator("#board")).toContainText("oldlane");
    await page
      .locator('[data-height-panel="board"] .qa-segmented button')
      .filter({ hasText: "Archived" })
      .click();
    await expect(page.locator("#board")).toContainText("oldlane");
    await expect(page.locator("#board")).not.toContainText("alpha");

    await expect(page.locator("#tasks")).toContainText("e2e-task");
    await expect(page.locator("#error")).not.toHaveClass(/visible/);
    await expect(page.locator("#events .event-row").first()).toBeVisible();
    await expect(page.locator("#details-col-splitter")).toBeVisible();
    await expect(page.locator(".dock-host")).toBeVisible();
    await expect(page.locator("#running-tasks")).toBeVisible();
  });

  test("TimeBadge cycle + running task highlight + dock persist", async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.setItem("porq.dash.view", "details");
      } catch {
        /* ignore */
      }
    });
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });
    await expect(page.locator("#view-details")).toHaveClass(/active/);
    await page.evaluate(() => {
      localStorage.removeItem("porq.dash.time.format");
      localStorage.removeItem("porq.dash.time.formats");
      localStorage.removeItem("porq.dash.layout.dock");
    });
    await page.reload();
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });

    const badges = page.locator("#events .time-badge");
    await expect(badges.first()).toBeVisible();
    const count = await badges.count();
    expect(count).toBeGreaterThanOrEqual(2);
    const badgeA = badges.nth(0);
    const badgeB = badges.nth(1);
    const beforeA = await badgeA.innerText();
    const beforeB = await badgeB.innerText();
    await badgeA.click();
    await expect
      .poll(async () =>
        page.evaluate(() => {
          const raw = localStorage.getItem("porq.dash.time.formats");
          if (!raw) return null;
          const map = JSON.parse(raw) as Record<string, string>;
          return Object.values(map).includes("abs24") ? "abs24" : null;
        })
      )
      .toBe("abs24");
    const afterA = await badgeA.innerText();
    const afterB = await badgeB.innerText();
    expect(afterA).not.toBe(beforeA);
    expect(afterB).toBe(beforeB);

    const idA = await badgeA.getAttribute("data-time-id");
    expect(idA).toBeTruthy();
    await page.reload();
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });
    await expect
      .poll(async () =>
        page.evaluate((id) => {
          const raw = localStorage.getItem("porq.dash.time.formats");
          if (!raw || !id) return null;
          return (JSON.parse(raw) as Record<string, string>)[id] || null;
        }, idA)
      )
      .toBe("abs24");

    await expect(page.locator("#running-tasks .running-card")).toHaveCount(1, { timeout: 15_000 });
    await page.locator("#running-tasks .running-card").first().click();
    await expect(page.locator("[data-testid=running-inspector]")).toBeVisible();
    await expect(page.locator("#board tr.related-to-selection")).toContainText("alpha");

    await page.evaluate(() => {
      const layout = {
        v: 1,
        colSplitPct: 54.5,
        columns: [
          [
            { tabs: ["ops-health"], active: 0, height: 320 },
            { tabs: ["jobs"], active: 0, height: 260 },
            { tabs: ["events"], active: 0, height: 300 },
          ],
          [
            { tabs: ["running-tasks"], active: 0, height: 220 },
            { tabs: ["tasks", "board"], active: 1, height: 260 },
            { tabs: ["aff"], active: 0, height: 260 },
            { tabs: ["files"], active: 0, height: 180 },
          ],
        ],
      };
      localStorage.setItem("porq.dash.layout.dock", JSON.stringify(layout));
    });
    await page.reload();
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });
    await expect(page.locator(".dock-leaf .dock-tabs")).toBeVisible();
    await expect(page.locator('.dock-tab[data-panel-tab="board"]')).toBeVisible();
  });

  test("theme picker sets data-qa-theme", async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.removeItem("porq.dash.qa-theme");
      } catch {
        /* ignore */
      }
    });
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", {
      timeout: 10_000,
    });
    await expect(page.locator("html")).toHaveAttribute("data-qa-theme", /dark|light/);

    await page.locator("#theme-select").selectOption("light");
    await expect(page.locator("html")).toHaveAttribute("data-qa-theme", "light");

    await page.locator("#theme-select").selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-qa-theme", "dark");
  });

  test("canvas grid edge resize persists", async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.removeItem("porq.dash.layout.canvases");
        localStorage.removeItem("porq.dash.view");
      } catch {
        /* ignore */
      }
    });
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });
    await expect(page.locator("#view-canvases")).toHaveClass(/active/);
    const cell = page.locator('.canvas-grid-cell[data-canvas-key="plan"]');
    await expect(cell).toBeVisible();
    const handle = cell.locator('[data-canvas-resize="s"]');
    const box = await handle.boundingBox();
    expect(box).toBeTruthy();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2 + 120, { steps: 8 });
    await page.mouse.up();
    await expect
      .poll(async () => {
        return page.evaluate(() => {
          const raw = localStorage.getItem("porq.dash.layout.canvases");
          if (!raw) return 0;
          const layout = JSON.parse(raw) as { items?: { plan?: { h?: number } } };
          return layout.items?.plan?.h ?? 0;
        });
      })
      .toBeGreaterThan(6);
  });

  test("dock leaf edge resize persists", async ({ page }) => {
    await page.addInitScript(() => {
      try {
        localStorage.setItem("porq.dash.view", "details");
        localStorage.removeItem("porq.dash.layout.dock");
      } catch {
        /* ignore */
      }
    });
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", { timeout: 10_000 });
    await expect(page.locator("#view-details")).toHaveClass(/active/);
    const leaf = page.locator('[data-dock-col="0"] [data-dock-leaf="0"]');
    await expect(leaf).toBeVisible();
    const before = await leaf.evaluate((el) =>
      parseInt(getComputedStyle(el).getPropertyValue("--dock-leaf-height"), 10)
    );
    const handle = leaf.locator("[data-dock-resize]");
    const box = await handle.boundingBox();
    expect(box).toBeTruthy();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2 + 80, { steps: 6 });
    await page.mouse.up();
    await expect
      .poll(async () => {
        return page.evaluate(() => {
          const raw = localStorage.getItem("porq.dash.layout.dock");
          if (!raw) return 0;
          const layout = JSON.parse(raw) as { columns?: { height?: number }[][] };
          return layout.columns?.[0]?.[0]?.height ?? 0;
        });
      })
      .toBeGreaterThan(before);
  });

  test("computer focus claim wait release", async ({ page }) => {
    await page.goto(baseURL + "/");
    await expect(page.locator("#stamp")).not.toHaveText("connecting…", {
      timeout: 10_000,
    });
    await expect(page.locator("#computer-focus-panel")).toBeVisible();
    await expect(page.locator("#cf-claim")).toBeVisible();

    await page.locator("#cf-claim").click();
    await expect
      .poll(async () => page.locator("#cf-state").innerText(), { timeout: 15_000 })
      .toBe("held");

    await page.locator("#cf-release").click();
    await expect
      .poll(async () => page.locator("#cf-state").innerText(), { timeout: 15_000 })
      .toMatch(/idle|—/);
  });
});
