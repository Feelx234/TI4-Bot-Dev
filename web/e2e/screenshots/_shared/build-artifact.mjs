#!/usr/bin/env node
// Builds <folder>/index.html from <folder>/manifest.json: the PNGs listed there are inlined as
// data URIs, so the page is one self-contained file that can be published as an artifact.
//   node build-artifact.mjs <artifact folder>
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const folder = resolve(process.argv[2] ?? ".");
const manifestPath = join(folder, "manifest.json");
if (!existsSync(manifestPath)) throw new Error(`no manifest.json in ${folder}`);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const esc = (text) =>
  String(text).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// Optional `groups`: [{ title, text, cards: [{ name, change, pairs: [{ label, before, after, note? }] }] }].
// Every pair is a before and an after image side by side (stacked on a narrow screen).
const image = (file, alt) => {
  const path = join(folder, file);
  if (!existsSync(path)) throw new Error(`missing screenshot ${file}; run the captures first`);
  return `<img alt="${esc(alt)}" loading="lazy" src="data:image/png;base64,${readFileSync(path).toString("base64")}">`;
};
const groups = (manifest.groups ?? [])
  .map(
    (group) =>
      `<section class="group"><h2>${esc(group.title)}</h2>${group.text ? `<p class="lede">${esc(group.text)}</p>` : ""}${group.cards
        .map(
          (card) =>
            `<article class="card"><h3>${esc(card.name)}</h3><p class="change">${esc(card.change)}</p>${card.pairs
              .map(
                (pair) =>
                  `<div class="pair"><p class="pair__label">${esc(pair.label)}</p><div class="pair__row"><figure><figcaption>Before</figcaption>${image(pair.before, `${card.name}, ${pair.label}, before`)}</figure><figure><figcaption>After</figcaption>${image(pair.after, `${card.name}, ${pair.label}, after`)}</figure></div>${pair.note ? `<p class="pair__note">${esc(pair.note)}</p>` : ""}</div>`,
              )
              .join("")}</article>`,
        )
        .join("")}</section>`,
  )
  .join("\n  ");

const figures = (manifest.shots ?? [])
  .map((shot) => {
    const file = join(folder, shot.file);
    if (!existsSync(file)) throw new Error(`missing screenshot ${shot.file}; run the captures first`);
    const data = readFileSync(file).toString("base64");
    return `<section>${shot.heading ? `<h2>${esc(shot.heading)}</h2>` : ""}<figure><img alt="${esc(shot.alt ?? shot.caption)}" src="data:image/png;base64,${data}"><figcaption>${esc(shot.caption)}</figcaption></figure></section>`;
  })
  .join("\n  ");
const notes = manifest.notes?.length
  ? `<section><h2>${esc(manifest.notesHeading ?? "Notes")}</h2><ul class="notes">${manifest.notes
      .map((n) => `<li><b>${esc(n.title)}</b>${esc(n.text)}</li>`)
      .join("")}</ul></section>`
  : "";

const html = readFileSync(join(here, "artifact.tpl.html"), "utf8")
  .replaceAll("{{TITLE}}", esc(manifest.title))
  .replaceAll("{{DESCRIPTION}}", esc(manifest.description))
  .replaceAll("{{TAG}}", esc(manifest.tag ?? "captured by web/e2e/screenshots"))
  .replace("{{SHOTS}}", () => groups + figures)
  .replace("{{NOTES}}", () => notes);
writeFileSync(join(folder, "index.html"), html);
console.log(`built ${join(folder, "index.html")} (${Math.round(html.length / 1024)} KB, ${(manifest.shots ?? []).length + (manifest.groups ?? []).reduce((n, g) => n + g.cards.reduce((m, c) => m + c.pairs.length * 2, 0), 0)} shots)`);
