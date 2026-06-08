#!/usr/bin/env node
// Tiny registry for the single Workflow Board's multi-project list.
// The board (workflow-board.mjs) reads this file to render the project switcher.
//
// Usage:
//   node projects-registry.mjs upsert <name> <repoAbsPath>
//   node projects-registry.mjs remove <name>
//   node projects-registry.mjs list
//
// File location: $PROJECTS_JSON (default: ./projects.json next to this script).

import { readFileSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const HERE = dirname(fileURLToPath(import.meta.url));
const FILE = process.env.PROJECTS_JSON || join(HERE, 'projects.json');

const read = () => {
  try { const a = JSON.parse(readFileSync(FILE, 'utf8')); return Array.isArray(a) ? a : []; }
  catch { return []; }
};
const write = (a) => writeFileSync(FILE, JSON.stringify(a, null, 2) + '\n');

const [cmd, name, repo] = process.argv.slice(2);
let list = read();

if (cmd === 'upsert') {
  if (!name || !repo) { console.error('usage: upsert <name> <repoAbsPath>'); process.exit(2); }
  list = list.filter((p) => p.name !== name);
  list.push({ name, repo });
  list.sort((a, b) => a.name.localeCompare(b.name));
  write(list);
  console.log(`  registered project '${name}' → ${repo}  (${FILE})`);
} else if (cmd === 'remove') {
  if (!name) { console.error('usage: remove <name>'); process.exit(2); }
  const before = list.length;
  list = list.filter((p) => p.name !== name);
  write(list);
  console.log(`  ${before === list.length ? 'no such project' : 'removed project'} '${name}'  (${FILE})`);
} else if (cmd === 'list') {
  console.log(JSON.stringify(list, null, 2));
} else {
  console.error('usage: projects-registry.mjs upsert|remove|list ...');
  process.exit(2);
}
