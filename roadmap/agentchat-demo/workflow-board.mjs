#!/usr/bin/env node
// Workflow Board — a tiny, dependency-free viewer for the demo projects'
// issue→spec→plan→implement→review artifacts. Independent of agent-chat (does NOT
// touch its source or dashboard), so both upstreams stay pristine.
//
// MULTI-PROJECT: reads a registry ($PROJECTS_JSON = [{name,repo}]) and serves ONE page
// with a project switcher (tabs). One board, many projects. Falls back to a single
// $DEMO_REPO when no registry exists (backward compatible with the old single-team run).
//
// Per project it scans:
//   specs/project.spec.md, specs/task-*.spec.md
//   issues/NNN-*.md  ·  docs/plans/NNN-*.md  ·  docs/progress.md, docs/agent-knowledge.md
// and renders them as dark cards (Agent-Monitor style) with click-to-read.
//
// Usage:
//   PROJECTS_JSON=/path/projects.json PORT=8086 node workflow-board.mjs   # multi-project
//   DEMO_REPO=/path/to/repo node workflow-board.mjs                        # single project

import { createServer } from 'http';
import { readFileSync, readdirSync, statSync } from 'fs';
import { join, basename } from 'path';

const PORT = Number.parseInt(process.env.PORT || '8086', 10) || 8086;
const PROJECTS_JSON = process.env.PROJECTS_JSON || join(process.cwd(), 'projects.json');
const FALLBACK_REPO = process.env.DEMO_REPO || null;

// ── projects registry ─────────────────────────────────────────────────
// Read fresh on each request so add-team/down-team changes show without a restart.
function loadProjects() {
  let list = [];
  try { const a = JSON.parse(readFileSync(PROJECTS_JSON, 'utf8')); if (Array.isArray(a)) list = a; } catch {}
  list = list.filter((p) => p && p.name && p.repo);
  if (list.length === 0 && FALLBACK_REPO) list = [{ name: basename(FALLBACK_REPO), repo: FALLBACK_REPO }];
  return list;
}
const repoFor = (projects, name) => {
  const p = projects.find((x) => x.name === name) || projects[0];
  return p ? p.repo : null;
};

// ── tiny helpers ──────────────────────────────────────────────────────
const esc = (s) => String(s).replace(/[&<>"']/g, (c) =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

function safeRead(p) { try { return readFileSync(p, 'utf8'); } catch { return null; } }
function safeList(dir) { try { return readdirSync(dir); } catch { return []; } }
function mtime(p) { try { return statSync(p).mtimeMs; } catch { return 0; } }

// Pull a title (first `# ...`) and a `- **Status:** ...` line if present.
function meta(md, fallbackTitle) {
  const titleM = md && md.match(/^#\s+(.+)$/m);
  const statusM = md && md.match(/^[-*]\s*\*\*Status:?\*\*\s*(.+)$/im);
  return {
    title: titleM ? titleM[1].trim() : fallbackTitle,
    status: statusM ? statusM[1].trim() : null,
  };
}

// Issue type taxonomy (conventional-commits-ish). The coordinator writes an
// explicit `- **Type:** <type>` line at /create-issue time (authoritative); for
// legacy/untagged issues we infer from the title so old cards still get a badge.
const TYPE_DEFS = {
  feat:     { label: 'feat',     color: '#3fb950' }, // green  — new capability
  bug:      { label: 'bug',      color: '#f85149' }, // red    — defect / fix
  docs:     { label: 'docs',     color: '#58a6ff' }, // blue   — documentation
  refactor: { label: 'refactor', color: '#a371f7' }, // purple — no behavior change
  chore:    { label: 'chore',    color: '#8b949e' }, // gray   — tooling/build/deps
  test:     { label: 'test',     color: '#db6d28' }, // orange — tests only
  perf:     { label: 'perf',     color: '#39c5cf' }, // teal   — performance
};
// Map common synonyms onto the canonical keys.
const TYPE_SYNONYMS = {
  fix: 'bug', bugfix: 'bug', defect: 'bug', feature: 'feat', enhancement: 'feat',
  documentation: 'docs', doc: 'docs', performance: 'perf', style: 'chore', ci: 'chore', build: 'chore',
};
function normType(t) {
  if (!t) return null;
  const k = String(t).trim().toLowerCase();
  const c = TYPE_SYNONYMS[k] || k;
  return TYPE_DEFS[c] ? c : null;
}
// Decide an issue's type: explicit `**Type:**` line wins; else infer from TITLE
// only (not the body — review status text can contain "reject/bug" noise).
function issueType(md, title = '') {
  // tolerate leading whitespace so an inadvertently-indented metadata line still parses
  const m = md && md.match(/^\s*[-*]\s*\*\*Type:?\*\*\s*([A-Za-z]+)/im);
  const explicit = normType(m && m[1]);
  if (explicit) return explicit;
  const s = String(title).toLowerCase();
  if (/\b(bug|fix|broken|crash|regress|contrast|wrong|incorrect|defect)\b|闪退|崩溃|修复|对比度/.test(s)) return 'bug';
  if (/\b(docs?|documentation|readme)\b|文档/.test(s)) return 'docs';
  if (/\brefactor\b|重构/.test(s)) return 'refactor';
  if (/\b(test|tests)\b|测试/.test(s)) return 'test';
  if (/\bperf|performance\b|性能/.test(s)) return 'perf';
  if (/\b(chore|build|ci|deps?|bump)\b/.test(s)) return 'chore';
  return 'feat'; // default: most issues describe a new capability
}

// Minimal, safe Markdown → HTML (headings, code, lists, bold/code-span, hr).
function mdToHtml(md) {
  const lines = (md || '').split('\n');
  let out = '', inCode = false, inList = false;
  const inline = (t) => esc(t)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  for (const raw of lines) {
    if (raw.startsWith('```')) {
      if (inList) { out += '</ul>'; inList = false; }
      inCode = !inCode; out += inCode ? '<pre><code>' : '</code></pre>'; continue;
    }
    if (inCode) { out += esc(raw) + '\n'; continue; }
    if (/^\s*[-*]\s+/.test(raw)) {
      if (!inList) { out += '<ul>'; inList = true; }
      out += '<li>' + inline(raw.replace(/^\s*[-*]\s+/, '')) + '</li>'; continue;
    }
    if (inList) { out += '</ul>'; inList = false; }
    const h = raw.match(/^(#{1,4})\s+(.+)$/);
    if (h) { const n = h[1].length; out += `<h${n}>${inline(h[2])}</h${n}>`; continue; }
    if (/^\s*---\s*$/.test(raw)) { out += '<hr/>'; continue; }
    if (raw.trim() === '') { out += '<br/>'; continue; }
    out += '<p>' + inline(raw) + '</p>';
  }
  if (inList) out += '</ul>';
  if (inCode) out += '</code></pre>';
  return out;
}

// ── collect artifacts for ONE project repo ────────────────────────────
function collect(repo) {
  if (!repo) return [];
  const sections = [];

  // Project (project.spec.md)
  const projPath = join(repo, 'specs', 'project.spec.md');
  const projMd = safeRead(projPath);
  sections.push({
    key: 'project', label: 'Project',
    items: projMd ? [{ id: 'specs/project.spec.md', ...meta(projMd, 'project.spec.md'),
      status: 'contract', mtime: mtime(projPath) }] : [],
  });

  // Issues
  const issuesDir = join(repo, 'issues');
  const issues = safeList(issuesDir).filter((f) => f.endsWith('.md')).sort()
    .map((f) => { const md = safeRead(join(issuesDir, f)); const mo = meta(md, f);
      // infer from title + filename slug (slug often carries the signal, e.g. "…-contrast")
      return { id: `issues/${f}`, ...mo, type: issueType(md, `${mo.title} ${f}`), mtime: mtime(join(issuesDir, f)) }; });
  sections.push({ key: 'issues', label: 'Issues', items: issues });

  // Specs — ALL .spec.md in specs/ (project.spec.md tagged 'project',
  // task-*.spec.md tagged 'task').
  const specsDir = join(repo, 'specs');
  const specFiles = safeList(specsDir).filter((f) => f.endsWith('.spec.md')).sort();
  const specs = specFiles.map((f) => {
    const md = safeRead(join(specsDir, f));
    const kind = /^task-/.test(f) ? 'task' : (f === 'project.spec.md' ? 'project' : 'spec');
    return { id: `specs/${f}`, ...meta(md, f), status: kind, mtime: mtime(join(specsDir, f)) };
  });
  sections.push({ key: 'specs', label: 'Specs', items: specs });

  // Plans
  const plansDir = join(repo, 'docs', 'plans');
  const plans = safeList(plansDir).filter((f) => f.endsWith('.md')).sort()
    .map((f) => { const md = safeRead(join(plansDir, f));
      return { id: `docs/plans/${f}`, ...meta(md, f), mtime: mtime(join(plansDir, f)) }; });
  sections.push({ key: 'plans', label: 'Plans', items: plans });

  // Notes (docs/progress.md, docs/agent-knowledge.md)
  const docsDir = join(repo, 'docs');
  const notes = safeList(docsDir).filter((f) => f.endsWith('.md')).sort()
    .map((f) => { const md = safeRead(join(docsDir, f));
      return { id: `docs/${f}`, ...meta(md, f), status: 'notes', mtime: mtime(join(docsDir, f)) }; });
  sections.push({ key: 'notes', label: 'Agent notes', items: notes });

  return sections;
}

function readArtifact(relId, repo) {
  // relId is repo-relative, restrict to known subdirs to avoid traversal.
  if (!repo || !/^(specs|issues|docs)\//.test(relId) || relId.includes('..')) return null;
  return safeRead(join(repo, relId));
}

// ── HTML shell (Agent-Monitor dark style) ─────────────────────────────
const STATUS_COLOR = (s = '') => {
  const t = s.toLowerCase();
  if (/done|complete|approved|pass/.test(t)) return '#3fb950';
  if (/await|open|drafting|pending|wait/.test(t)) return '#d29922';
  if (/review|implement|progress|planning/.test(t)) return '#58a6ff';
  if (/reject|fail|block/.test(t)) return '#f85149';
  return '#8b949e';
};

function page(sections, projects, active) {
  const typeChip = (it) => (it.type && TYPE_DEFS[it.type])
    ? `<span class="type" style="--tc:${TYPE_DEFS[it.type].color}">${esc(TYPE_DEFS[it.type].label)}</span>` : '';
  const card = (it) => `
    <div class="card" onclick="openDoc('${esc(it.id)}')">
      <div class="card-title">${typeChip(it)}${esc(it.title || it.id)}</div>
      <div class="card-meta">
        <span class="path">${esc(it.id)}</span>
        ${it.status ? `<span class="status" style="--c:${STATUS_COLOR(it.status)}">${esc(it.status)}</span>` : ''}
      </div>
    </div>`;
  const col = (s) => `
    <section class="col">
      <h2>${esc(s.label)} <span class="count">${s.items.length}</span></h2>
      ${s.items.length ? s.items.map(card).join('') : '<div class="empty">—</div>'}
    </section>`;
  const activeRepo = repoFor(projects, active) || '(no project)';
  const tabs = projects.length
    ? projects.map((p) => `<a class="tab${p.name === active ? ' active' : ''}" href="/?project=${encodeURIComponent(p.name)}">${esc(p.name)}</a>`).join('')
    : '<span class="no-proj">No projects yet — run <code>add-team.sh</code></span>';
  const boardHtml = projects.length
    ? sections.map(col).join('')
    : '<div class="empty" style="padding:24px">No project selected.</div>';
  return `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Workflow Board — agentchat demo</title>
<style>
  :root{ --bg:#0d1117; --panel:#161b22; --border:#30363d; --fg:#c9d1d9; --dim:#8b949e; --accent:#58a6ff; }
  *{box-sizing:border-box} body{margin:0;background:var(--bg);color:var(--fg);
    font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif}
  header{padding:14px 20px;border-bottom:1px solid var(--border);display:flex;align-items:center;gap:14px}
  header h1{font-size:16px;margin:0;font-weight:600}
  header .sub{color:var(--dim);font-size:12px}
  header a.monitor{color:var(--accent);text-decoration:none;font-size:12px;margin-left:auto}
  .tabs{display:flex;gap:6px;padding:8px 20px 0;flex-wrap:wrap;border-bottom:1px solid var(--border);align-items:flex-end}
  .tab{padding:6px 12px;border:1px solid var(--border);border-bottom:none;border-radius:6px 6px 0 0;
    color:var(--dim);text-decoration:none;font-size:12px;background:var(--panel)}
  .tab.active{color:var(--fg);border-color:var(--accent);background:var(--bg);font-weight:600}
  .tab:hover{color:var(--fg)}
  .no-proj{color:var(--dim);font-size:12px;padding:6px 0}
  .board{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:14px;padding:18px}
  .col h2{font-size:12px;text-transform:uppercase;letter-spacing:.5px;color:var(--dim);
    margin:0 0 10px;display:flex;align-items:center;gap:8px}
  .col .count{background:var(--panel);border:1px solid var(--border);border-radius:10px;
    padding:0 7px;font-size:11px;color:var(--fg)}
  .card{background:var(--panel);border:1px solid var(--border);border-radius:8px;padding:11px 12px;
    margin-bottom:10px;cursor:pointer;transition:border-color .15s}
  .card:hover{border-color:var(--accent)}
  .card-title{font-weight:600;margin-bottom:6px;color:var(--fg)}
  .type{display:inline-block;font-size:10px;font-weight:700;text-transform:uppercase;letter-spacing:.4px;
    background:var(--tc);color:#0d1117;border-radius:10px;padding:1px 7px;margin-right:7px;vertical-align:1px}
  .card-meta{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
  .card-meta .path{color:var(--dim);font-size:11px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
  .status{font-size:11px;border:1px solid var(--c);color:var(--c);border-radius:10px;padding:0 7px}
  .empty{color:var(--dim);padding:8px 0}
  /* modal */
  #ov{position:fixed;inset:0;background:rgba(0,0,0,.6);display:none;align-items:flex-start;justify-content:center;padding:40px}
  #ov.on{display:flex}
  #doc{background:var(--panel);border:1px solid var(--border);border-radius:10px;max-width:860px;width:100%;
    max-height:85vh;overflow:auto;padding:24px 30px}
  #doc h1{font-size:20px} #doc h2{font-size:16px;border-bottom:1px solid var(--border);padding-bottom:4px}
  #doc h3{font-size:14px} #doc code{background:#0d1117;border:1px solid var(--border);border-radius:4px;padding:1px 5px;font-size:12px}
  #doc pre{background:#0d1117;border:1px solid var(--border);border-radius:6px;padding:12px;overflow:auto}
  #doc pre code{border:0;padding:0} #doc hr{border:0;border-top:1px solid var(--border)}
  #doc .head{display:flex;align-items:center;margin-bottom:12px}
  #doc .head .x{margin-left:auto;cursor:pointer;color:var(--dim);font-size:20px;border:0;background:0}
  #doc .head .fp{color:var(--dim);font-size:12px;font-family:ui-monospace,monospace}
</style></head><body>
<header>
  <h1>📋 Workflow Board</h1>
  <span class="sub">${esc(activeRepo)} · issue → spec → plan → implement → review</span>
  <a class="monitor" href="http://127.0.0.1:8084" target="_blank">Agent Monitor →</a>
</header>
<nav class="tabs">${tabs}</nav>
<div class="board">${boardHtml}</div>
<div id="ov" onclick="if(event.target.id==='ov')closeDoc()">
  <div id="doc"><div class="head"><span class="fp" id="fp"></span><button class="x" onclick="closeDoc()">×</button></div>
  <div id="body"></div></div>
</div>
<script>
const PROJECT = ${JSON.stringify(active || '')};
async function openDoc(id){
  const r = await fetch('/doc?id='+encodeURIComponent(id)+'&project='+encodeURIComponent(PROJECT));
  const j = await r.json();
  document.getElementById('fp').textContent = id;
  document.getElementById('body').innerHTML = j.html || '<p>(empty)</p>';
  document.getElementById('ov').classList.add('on');
}
function closeDoc(){ document.getElementById('ov').classList.remove('on'); }
document.addEventListener('keydown',e=>{ if(e.key==='Escape')closeDoc(); });
// auto-refresh the SELECTED project's cards every 5s (cheap, no framework)
setInterval(async()=>{ try{ const r=await fetch('/?project='+encodeURIComponent(PROJECT));
  const t=await r.text(); const m=t.match(/<div class="board">([\\s\\S]*?)<\\/div>\\s*<div id="ov"/);
  if(m){ document.querySelector('.board').innerHTML=m[1]; } }catch{} }, 5000);
</script>
</body></html>`;
}

// ── server ────────────────────────────────────────────────────────────
const server = createServer((req, res) => {
  const u = new URL(req.url, `http://127.0.0.1:${PORT}`);
  const projects = loadProjects();
  if (u.pathname === '/doc') {
    const id = u.searchParams.get('id') || '';
    const repo = repoFor(projects, u.searchParams.get('project') || '');
    const md = readArtifact(id, repo);
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(md == null ? { error: 'not found' } : { html: mdToHtml(md) }));
    return;
  }
  const active = u.searchParams.get('project') || (projects[0] && projects[0].name) || '';
  const repo = repoFor(projects, active);
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(page(collect(repo), projects, active));
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`Workflow Board running on http://127.0.0.1:${PORT}  (registry=${PROJECTS_JSON}${FALLBACK_REPO ? `, fallback=${FALLBACK_REPO}` : ''})`);
});
