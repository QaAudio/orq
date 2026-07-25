import { nextTick, watch, type Ref, type WatchSource } from "vue";

let mermaidMod: typeof import("mermaid") | null = null;
let lastTheme: string | null = null;
let runGen = 0;

async function ensureMermaid(theme: "dark" | "light") {
  if (!mermaidMod) {
    mermaidMod = await import("mermaid");
  }
  if (theme !== lastTheme) {
    mermaidMod.default.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: theme === "light" ? "neutral" : "dark",
    });
    lastTheme = theme;
  }
  return mermaidMod.default;
}

/** Render pending `.canvas-md .mermaid` nodes (idempotent per generation). */
export async function runMermaidIn(
  root: ParentNode | null | undefined,
  theme: "dark" | "light"
) {
  const scope = root || document;
  const nodes = Array.from(
    scope.querySelectorAll<HTMLElement>(".canvas-md pre.mermaid")
  ).filter((el) => el.textContent?.trim() && !el.querySelector("svg"));
  if (!nodes.length) return;
  const gen = ++runGen;
  const mermaid = await ensureMermaid(theme);
  if (gen !== runGen) return;
  try {
    await mermaid.run({ nodes });
  } catch {
    /* leave source visible on parse errors */
  }
}

function resetMermaidNodes(scope: ParentNode) {
  for (const el of scope.querySelectorAll<HTMLElement>(".canvas-md pre.mermaid")) {
    const svg = el.querySelector("svg");
    if (svg) svg.remove();
    el.removeAttribute("data-processed");
  }
}

/** Watch markdown HTML + theme; re-run Mermaid after paint. */
export function useMermaid(
  source: WatchSource | Ref<unknown>,
  theme: Ref<"dark" | "light">
) {
  watch(
    source,
    async () => {
      await nextTick();
      await runMermaidIn(document.getElementById("canvases"), theme.value);
    },
    { flush: "post" }
  );

  watch(theme, async () => {
    lastTheme = null;
    const scope = document.getElementById("canvases");
    if (!scope) return;
    resetMermaidNodes(scope);
    await nextTick();
    await runMermaidIn(scope, theme.value);
  });
}
