#!/usr/bin/env node
// Pre-create the Matrix accounts the demo needs, using Palpo's m.login.dummy flow.
//
// WHY THIS EXISTS: agent-chat's bridge auto-register path (bridge-matrix.js
// matrixRegister) hard-requires MATRIX_REG_TOKEN and only does the
// m.login.registration_token flow. Local Palpo here uses the m.login.dummy flow
// (no token). The bridge tries matrixLogin FIRST and only falls back to register,
// so if we pre-create every account with the dummy flow, the bridge just logs in
// and never hits its broken register path.
//
// It derives each agent's password EXACTLY like the bridge does:
//   sha256(`${MATRIX_AGENT_PASSWORD_SECRET}:${agentName}`).hex
// and registers the bot with MATRIX_BOT_PASSWORD verbatim.
//
// Usage:
//   # reads agent-chat/.env for secrets (or pass via env)
//   node register-accounts.mjs
//   AGENTS="wf_coordinator wf_implementer wf_reviewer" node register-accounts.mjs
//
// Idempotent: an already-existing account (M_USER_IN_USE) is treated as OK.

import { createHash } from 'crypto';
import { readFileSync } from 'fs';

const HS = (process.env.MATRIX_HOMESERVER || 'http://127.0.0.1:8128').replace(/\/$/, '');
const SN = process.env.MATRIX_SERVER_NAME || '127.0.0.1:8128';
const ENV_FILE = process.env.ENV_FILE
  || `${process.env.AC_DIR || '/Users/zhangalex/Work/Projects/consult/agent-chat'}/.env`;

// Load KEY=value from the .env file unless already in process.env.
function loadEnv(file) {
  let txt = '';
  try { txt = readFileSync(file, 'utf8'); } catch { return {}; }
  const out = {};
  for (const line of txt.split('\n')) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*)$/);
    if (!m) continue;
    let v = m[2].replace(/\s+#.*$/, '').trim();
    out[m[1]] = v;
  }
  return out;
}
const fileEnv = loadEnv(ENV_FILE);
const val = (k, d = '') => (process.env[k] ?? fileEnv[k] ?? d).trim?.() ?? d;

const PREFIX   = val('MATRIX_AGENT_PREFIX', 'ac_');
const BOT_USER = val('MATRIX_BOT_USERNAME', 'agent-bridge');
const BOT_PASS = val('MATRIX_BOT_PASSWORD');
const SECRET   = val('MATRIX_AGENT_PASSWORD_SECRET');
// AGENTS env overrides the default list (multi-team: pass the team's 4 agent names).
// An EXPLICITLY-EMPTY `AGENTS=""` means "bot account only" — used by start-infra.sh to
// ensure @agent-bridge exists before any team is added. Only fall back to the default
// 4-agent list when AGENTS is UNSET. (wf_final_reviewer is the Codex final-gate agent;
// runtime doesn't matter for accounts — it just needs a Matrix account to be invited.)
const AGENTS_RAW = process.env.AGENTS !== undefined
  ? process.env.AGENTS
  : 'wf_coordinator wf_implementer wf_reviewer wf_final_reviewer';
const AGENTS = AGENTS_RAW.split(/\s+/).filter(Boolean);

function isPlaceholder(s) { return !s || /[<>]/.test(s); }
function deriveAgentPassword(name) {
  return createHash('sha256').update(`${SECRET}:${name}`).digest('hex');
}

async function canLogin(username, password) {
  const r = await fetch(`${HS}/_matrix/client/v3/login`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ type: 'm.login.password', identifier: { type: 'm.id.user', user: username }, password }),
  });
  return r.status === 200;
}

// Set a FRIENDLY display name (the un-prefixed canonical name, e.g. "wf_coordinator").
// WHY: the account localpart is `ac_wf_coordinator`, and registration defaults the
// display name to that localpart. robrix2's @-mention autocomplete prefix-matches on
// display name FIRST — so with the `ac_` prefix, typing `@wf_coordinator` only hits the
// lowest-priority "contains" tier and the agent is buried. A clean `wf_coordinator`
// display name makes `@wf_…` a top-priority prefix match. (The bridge only sets a
// display name on accounts IT creates; since we pre-create here, we must set it too.)
async function ensureDisplayName(username, password, displayName) {
  const lg = await fetch(`${HS}/_matrix/client/v3/login`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ type: 'm.login.password', identifier: { type: 'm.id.user', user: username }, password }),
  }).then(r => r.json()).catch(() => ({}));
  if (!lg.access_token) return false;
  const mxid = `@${username}:${SN}`;
  const r = await fetch(`${HS}/_matrix/client/v3/profile/${encodeURIComponent(mxid)}/displayname`, {
    method: 'PUT', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${lg.access_token}` },
    body: JSON.stringify({ displayname: displayName }),
  }).catch(() => ({ status: 0 }));
  return r.status === 200;
}

async function register(username, password) {
  // Step 1: probe for the session + supported flows.
  const probe = await fetch(`${HS}/_matrix/client/v3/register`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  const pj = await probe.json();
  if (pj.access_token) return { ok: true, mxid: pj.user_id, note: 'created (no UIA)' };
  // Already exists: VERIFY the stored password matches what the bridge will use,
  // else the bridge's login will fail and it can't self-register here. This is the
  // exact mismatch that silently breaks the demo if an account was made earlier
  // with a different MATRIX_AGENT_PASSWORD_SECRET.
  if (pj.errcode === 'M_USER_IN_USE') {
    const good = await canLogin(username, password);
    return good
      ? { ok: true, mxid: `@${username}:${SN}`, note: 'already exists (password OK)' }
      : { ok: false, err: `exists but password MISMATCH — was it created with a different MATRIX_AGENT_PASSWORD_SECRET / MATRIX_BOT_PASSWORD? Reset it on Palpo or pick a fresh agent name.` };
  }
  const session = pj.session;
  const flows = (pj.flows || []).map(f => (f.stages || []).join('+'));
  if (!session) return { ok: false, err: `no session; resp=${JSON.stringify(pj)}` };
  if (!flows.some(f => f.includes('m.login.dummy'))) {
    return { ok: false, err: `server wants ${JSON.stringify(flows)} (not dummy). Set MATRIX_REG_TOKEN flow manually.` };
  }
  // Step 2: complete with the dummy stage.
  const res = await fetch(`${HS}/_matrix/client/v3/register`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password, auth: { type: 'm.login.dummy', session } }),
  });
  const rj = await res.json();
  if (rj.access_token) return { ok: true, mxid: rj.user_id, note: 'created' };
  if (rj.errcode === 'M_USER_IN_USE') {
    const good = await canLogin(username, password);
    return good ? { ok: true, mxid: `@${username}:${SN}`, note: 'already exists (password OK)' }
                : { ok: false, err: 'exists but password MISMATCH after race — reset on Palpo.' };
  }
  return { ok: false, err: JSON.stringify(rj) };
}

const problems = [];
if (isPlaceholder(BOT_PASS)) problems.push('MATRIX_BOT_PASSWORD is unset/placeholder');
if (isPlaceholder(SECRET))   problems.push('MATRIX_AGENT_PASSWORD_SECRET is unset/placeholder');
if (problems.length) {
  console.error(`[register-accounts] cannot proceed:\n  - ${problems.join('\n  - ')}\n` +
    `Fill them in ${ENV_FILE} (or pass via env) and re-run.`);
  process.exit(2);
}

console.log(`[register-accounts] homeserver=${HS}`);
console.log(`[register-accounts] bot=@${BOT_USER}  agents=${AGENTS.map(a => '@' + PREFIX + a).join(', ')}\n`);

let failed = 0;
// Bot account (verbatim password).
{
  const r = await register(BOT_USER, BOT_PASS);
  console.log(`${r.ok ? '✓' : '✗'} @${BOT_USER} — ${r.ok ? r.note : r.err}`);
  if (!r.ok) failed++;
}
// Agent accounts (derived passwords, must match the bridge).
for (const a of AGENTS) {
  const pw = deriveAgentPassword(a);
  const r = await register(`${PREFIX}${a}`, pw);
  let note = r.ok ? r.note : r.err;
  if (r.ok) {
    // Make `@${a}` (e.g. @wf_coordinator) cleanly mentionable in robrix2.
    const named = await ensureDisplayName(`${PREFIX}${a}`, pw, a);
    note += named ? `, displayname="${a}"` : ', displayname set FAILED (mention still works via contains-match)';
  }
  console.log(`${r.ok ? '✓' : '✗'} @${PREFIX}${a} — ${note}`);
  if (!r.ok) failed++;
}

console.log(failed
  ? `\n[register-accounts] ${failed} account(s) failed — see errors above.`
  : `\n[register-accounts] all accounts ready. The bridge will log in (not register) on start.`);
process.exit(failed ? 1 : 0);
