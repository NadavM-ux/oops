// Downloads the Hebrew + Latin subsets of IBM Plex Sans Hebrew (and Plex Mono for code)
// and writes a self-hosted fonts.css. Run once; results are committed.
import fs from "node:fs/promises";
import path from "node:path";

const [cssFile, outDir] = process.argv.slice(2);
const css = await fs.readFile(cssFile, "utf8");
const keep = new Set(["hebrew", "latin", "latin-ext"]);
const blocks = [...css.matchAll(/\/\* ([\w-]+) \*\/\s*@font-face \{([^}]*)\}/g)];
let out = "/* IBM Plex Sans Hebrew & IBM Plex Mono, SIL Open Font License 1.1 */\n";
for (const [, subset, body] of blocks) {
  if (!keep.has(subset)) continue;
  const family = body.match(/font-family: '([^']+)'/)[1];
  const weight = body.match(/font-weight: (\d+)/)[1];
  if (family === "IBM Plex Mono" && subset !== "latin") continue;
  const url = body.match(/url\((https:[^)]+)\)/)[1];
  const range = body.match(/unicode-range: ([^;]+);/)[1];
  const file = `${family.replace(/ /g, "")}-${weight}-${subset}.woff2`;
  const res = await fetch(url);
  await fs.writeFile(path.join(outDir, file), Buffer.from(await res.arrayBuffer()));
  out += `@font-face{font-family:'${family}';font-style:normal;font-weight:${weight};font-display:swap;src:url('${file}') format('woff2');unicode-range:${range};}\n`;
}
await fs.writeFile(path.join(outDir, "fonts.css"), out);
console.log("wrote", (await fs.readdir(outDir)).length, "files");
