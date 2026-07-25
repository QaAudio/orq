function esc(s: string): string {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function inlineFormat(s: string): string {
  return s
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" rel="noopener noreferrer">$1</a>');
}

/** Safe escape-first markdown subset (parity with legacy dash). */
export function renderMarkdown(src: string): string {
  const lines = String(src || "").replace(/\r\n/g, "\n").split("\n");
  const out: string[] = [];
  let i = 0;
  let inCode = false;
  let codeLang = "";
  const codeBuf: string[] = [];
  let listType: "ul" | "ol" | null = null;

  function flushList() {
    if (listType) {
      out.push(`</${listType}>`);
      listType = null;
    }
  }

  function flushCode() {
    const body = codeBuf.join("\n");
    if (codeLang === "mermaid") {
      out.push('<pre class="mermaid">' + body + "</pre>");
    } else {
      out.push("<pre><code>" + body + "</code></pre>");
    }
    codeBuf.length = 0;
    codeLang = "";
    inCode = false;
  }

  function flushTable(rows: string[][]) {
    if (!rows.length) return;
    const head = rows[0];
    out.push("<table><thead><tr>");
    for (const c of head) out.push(`<th>${inlineFormat(esc(c.trim()))}</th>`);
    out.push("</tr></thead><tbody>");
    for (const row of rows.slice(1)) {
      out.push("<tr>");
      for (const c of row) out.push(`<td>${inlineFormat(esc(c.trim()))}</td>`);
      out.push("</tr>");
    }
    out.push("</tbody></table>");
  }

  while (i < lines.length) {
    const line = lines[i];

    if (line.startsWith("```")) {
      if (inCode) {
        flushCode();
      } else {
        flushList();
        inCode = true;
        codeLang = line.slice(3).trim().split(/\s+/)[0]?.toLowerCase() || "";
      }
      i += 1;
      continue;
    }
    if (inCode) {
      codeBuf.push(esc(line));
      i += 1;
      continue;
    }

    if (line.includes("|") && /^\s*\|/.test(line)) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].includes("|")) {
        const raw = lines[i].trim();
        if (/^\|?\s*:?-{3,}/.test(raw)) {
          i += 1;
          continue;
        }
        const cells = raw
          .replace(/^\|/, "")
          .replace(/\|$/, "")
          .split("|");
        rows.push(cells);
        i += 1;
      }
      flushList();
      flushTable(rows);
      continue;
    }

    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      flushList();
      const level = heading[1].length;
      out.push(`<h${level}>${inlineFormat(esc(heading[2]))}</h${level}>`);
      i += 1;
      continue;
    }

    const ul = /^[-*]\s+(.*)$/.exec(line);
    if (ul) {
      if (listType !== "ul") {
        flushList();
        out.push("<ul>");
        listType = "ul";
      }
      out.push("<li>" + inlineFormat(esc(ul[1])) + "</li>");
      i += 1;
      continue;
    }

    const ol = /^(\d+)\.\s+(.*)$/.exec(line);
    if (ol) {
      if (listType !== "ol") {
        flushList();
        out.push("<ol>");
        listType = "ol";
      }
      out.push("<li>" + inlineFormat(esc(ol[2])) + "</li>");
      i += 1;
      continue;
    }

    flushList();
    if (!line.trim()) out.push("");
    else out.push("<p>" + inlineFormat(esc(line)) + "</p>");
    i += 1;
  }
  if (inCode) flushCode();
  flushList();
  return out.join("\n");
}
