#!/usr/bin/env node
/**
 * Extract the autoeq ExportFormat enum into a JSON file the site can import.
 *
 * Source of truth: crates/autoeq/src/roomeq/export.rs
 * Output:          site/src/data/export-formats.json
 *
 * Parsing strategy: read the file as text, locate the `enum ExportFormat { … }`
 * block, and walk it line-by-line collecting (doc comment, #[value name],
 * variant ident) triples plus the matching `default_extension()` arms.
 *
 * Failures (file missing, regex mismatch) are fatal — the marketing site lies
 * if this list drifts, so we'd rather break the build than ship stale data.
 */
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SOURCE = resolve(__dirname, '../../crates/autoeq/src/roomeq/export.rs');
const OUTPUT = resolve(__dirname, '../src/data/export-formats.json');

// Display label for each variant (kebab-case readable names). Keep this map
// in sync if the enum gains new variants — the script will throw if a variant
// has no entry here, which is the signal that this needs updating.
const LABELS = {
  CamillaDsp: 'CamillaDSP',
  EqualizerApo: 'Equalizer APO',
  EasyEffects: 'EasyEffects',
  Wavelet: 'Wavelet',
  PipeWire: 'PipeWire',
  RoonDsp: 'Roon',
};

function fail(msg) {
  console.error(`gen-export-formats: ${msg}`);
  process.exit(1);
}

const src = await readFile(SOURCE, 'utf8').catch(() => fail(`cannot read ${SOURCE}`));

// 1. Slice the enum body.
const enumMatch = src.match(/pub enum ExportFormat \{([\s\S]*?)^\}/m);
if (!enumMatch) fail('could not find `pub enum ExportFormat { … }`');
const enumBody = enumMatch[1];

// 2. Walk the body. Each variant is preceded by a /// doc comment line and a
//    #[value(name = "…")] attribute. We anchor on the variant identifier and
//    look backwards for its doc + cli name.
const variants = [];
const lines = enumBody.split('\n');
let pendingDoc = null;
let pendingCliName = null;
for (const raw of lines) {
  const line = raw.trim();
  if (!line) continue;
  let m;
  if ((m = line.match(/^\/\/\/\s?(.*)$/))) {
    pendingDoc = (pendingDoc ? pendingDoc + ' ' : '') + m[1].trim();
    continue;
  }
  if ((m = line.match(/^#\[value\(name\s*=\s*"([^"]+)"\)\]$/))) {
    pendingCliName = m[1];
    continue;
  }
  if ((m = line.match(/^([A-Z][A-Za-z0-9]*)\s*,?\s*$/))) {
    const ident = m[1];
    variants.push({
      id: ident,
      cliName: pendingCliName,
      doc: pendingDoc,
    });
    pendingDoc = null;
    pendingCliName = null;
  }
}
if (variants.length === 0) fail('no variants parsed from ExportFormat');

// 3. Pull extensions from `default_extension()` for completeness.
const extBlockMatch = src.match(/fn default_extension\(&self\) -> &'static str \{([\s\S]*?)\}/);
if (!extBlockMatch) fail('could not find default_extension()');
const extBody = extBlockMatch[1];
const extByVariant = {};
for (const m of extBody.matchAll(/ExportFormat::([A-Za-z0-9]+)\s*=>\s*"([^"]+)"/g)) {
  extByVariant[m[1]] = m[2];
}

// 4. Decorate variants with the display label and extension. Throw if a
//    variant has no LABELS entry — that's the signal to update this script.
const out = variants.map((v) => {
  if (!(v.id in LABELS)) {
    fail(`no LABELS entry for variant ${v.id} — update site/scripts/gen-export-formats.mjs`);
  }
  return {
    id: v.id,
    cliName: v.cliName,
    label: LABELS[v.id],
    extension: extByVariant[v.id] ?? null,
    description: v.doc,
  };
});

const json = JSON.stringify(
  {
    source: 'crates/autoeq/src/roomeq/export.rs',
    generatedAt: new Date().toISOString(),
    formats: out,
  },
  null,
  2,
);

await writeFile(OUTPUT, json + '\n', 'utf8');
console.log(`gen-export-formats: wrote ${out.length} formats → ${OUTPUT}`);
for (const f of out) {
  console.log(`  - ${f.label.padEnd(16)} (${f.cliName}, .${f.extension})`);
}
