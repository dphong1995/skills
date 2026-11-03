#!/usr/bin/env bash
# Regenerate hud/omc/ (next to this script) from the OMC plugin cache.
# Usage: vendor.sh [version]   (default: highest version that has dist/hud/index.js)
# Copies the import closure of dist/hud/index.js plus package.json and LICENSE, keeping relative paths.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/plugins/cache/omc/oh-my-claudecode"

node - "$BASE" "$SRC/omc" "${1:-}" <<'NODE'
const fs = require("fs");
const path = require("path");
const { isBuiltin } = require("node:module");
const [base, dest, arg] = process.argv.slice(2);

let version = arg;
if (!version) {
  const vs = fs.existsSync(base)
    ? fs.readdirSync(base).filter((v) => fs.existsSync(path.join(base, v, "dist/hud/index.js")))
    : [];
  vs.sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  if (!vs.length) {
    console.error(`error: no built OMC version with dist/hud/index.js under ${base}`);
    process.exit(1);
  }
  version = vs[vs.length - 1];
}
const root = path.join(base, version);

// The second entry is loaded by dist/installer/index.js through a non-literal dynamic import.
const entries = ["dist/hud/index.js", "scripts/lib/state-lock.mjs"];
const seen = new Set();
const queue = entries.map((e) => path.join(root, e));
const patterns = [
  /\bimport\s*(?:[^;"'()]*?\bfrom\s*)?["']([^"']+)["']/g,
  /\bexport\s*[^;"'()]*?\bfrom\s*["']([^"']+)["']/g,
  /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g,
  /\brequire\s*\(\s*["']([^"']+)["']\s*\)/g,
];

function resolveRel(dir, spec) {
  const base = path.resolve(dir, spec);
  for (const c of [base, base + ".js", path.join(base, "index.js")]) {
    if (fs.existsSync(c) && fs.statSync(c).isFile()) return c;
  }
  return null;
}

while (queue.length) {
  const file = queue.shift();
  if (seen.has(file)) continue;
  seen.add(file);
  if (!fs.existsSync(file)) {
    console.error(`error: missing file ${path.relative(root, file)}`);
    process.exit(1);
  }
  const src = fs.readFileSync(file, "utf8");
  const rel = path.relative(root, file);
  if (/\bimport\s*\(\s*[^"'\s)]/.test(src)) console.log(`note: ${rel} has a non-literal dynamic import()`);
  const specs = new Set();
  const importSpecs = new Set();
  const requireSpecs = new Set();
  patterns.forEach((re, i) => {
    for (const m of src.matchAll(re)) {
      specs.add(m[1]);
      (i === 3 ? requireSpecs : importSpecs).add(m[1]);
    }
  });
  for (const spec of specs) {
    if (spec.startsWith(".")) {
      const r = resolveRel(path.dirname(file), spec);
      if (!r) {
        console.error(`error: ${rel}: cannot resolve relative import "${spec}"`);
        process.exit(1);
      }
      queue.push(r);
    } else if (!isBuiltin(spec)) {
      if (requireSpecs.has(spec) && !importSpecs.has(spec)) {
        console.log(`note: ${rel} requires "${spec}" (not vendored)`);
      } else {
        console.error(`error: ${rel}: non-builtin package import "${spec}"`);
        process.exit(1);
      }
    }
  }
}

const files = [...seen].map((f) => path.relative(root, f));
for (const extra of ["package.json", "LICENSE"]) {
  if (!fs.existsSync(path.join(root, extra))) {
    console.error(`error: missing ${extra} in ${root}`);
    process.exit(1);
  }
  files.push(extra);
}
fs.rmSync(dest, { recursive: true, force: true });
for (const f of files) {
  fs.mkdirSync(path.dirname(path.join(dest, f)), { recursive: true });
  fs.copyFileSync(path.join(root, f), path.join(dest, f));
}
console.log(`Vendored OMC ${version}: ${files.length} files -> ${dest}`);
NODE
