# Deployment Guide: Hermes Agent + Matrix

[中文版](01-deploying-hermes-with-matrix-zh.md)

> **Goal:** After following this guide, you will be able to chat with [Hermes Agent](https://github.com/NousResearch/Hermes-Agent) directly from Robrix.

## What is Hermes Agent?

[Hermes Agent](https://github.com/NousResearch/Hermes-Agent) is a self-hosted AI agent framework open-sourced by [Nous Research](https://nousresearch.com/), designed natively around function calling and tool use. It talks to many LLM providers (Nous Portal, OpenAI, Anthropic, Gemini, DeepSeek, …) and connects to chat platforms like Matrix, Telegram, Discord, and WhatsApp through a unified messaging gateway. In this guide, Hermes uses its **built-in Matrix adapter** to log in to your homeserver **as a regular user**, requiring no server-side configuration — the same "regular Matrix client" integration model OpenClaw uses, which means Robrix can see and talk to it with no extra setup.

## About This Guide

This guide is focused on the path of getting a Hermes bot **fully running in a local environment**: install Hermes first, then have it log in to your Matrix server as a regular user, and finally chat with it from Robrix. Along the way we spell out the **common gotchas** of this specific combo — for example, the fact that the connection address and the account suffix don't always match when you're talking to a local Palpo.

For deeper Hermes usage (full environment variable list, Session Model, other messaging platforms, advanced encryption), the official documentation is the best guide — we recommend reading it alongside this one, and skimming the official docs first is a good way to prepare:

- **Hermes official docs:** [hermes-agent.nousresearch.com/docs](https://hermes-agent.nousresearch.com/docs/)
- **Matrix adapter reference:** [messaging/matrix](https://hermes-agent.nousresearch.com/docs/user-guide/messaging/matrix)
- **Hermes GitHub:** [github.com/NousResearch/Hermes-Agent](https://github.com/NousResearch/Hermes-Agent)

This guide was tested against Hermes v0.11.0 (April 2026) + local Palpo + macOS arm64. Hermes iterates quickly; if field names or commands in a later version diverge from what's written here, trust the official docs.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Install and Configure Hermes](#2-install-and-configure-hermes)
3. [Log Hermes Into Matrix](#3-log-hermes-into-matrix)
4. [Test From Robrix](#4-test-from-robrix)
5. [Where to Look When Things Go Wrong](#5-where-to-look-when-things-go-wrong)
6. [Further Reading](#6-further-reading)

---

## 1. Prerequisites

| Requirement | Notes |
| --- | --- |
| **Matrix server** | Local Palpo ([deployment guide](../robrix-with-palpo-and-octos/01-deploying-palpo-and-octos.md)), matrix.org, or your own Synapse — any works |
| **Robrix** | [Getting Started with Robrix](../robrix/getting-started-with-robrix.md) |
| **Two Matrix accounts** | One for yourself, one for the Hermes bot |
| **An LLM API key** | [DeepSeek](https://platform.deepseek.com/api_keys), Nous Portal, OpenAI, Anthropic, … take your pick |

We'll install Hermes itself in the next section — no need to pre-install.

---

## 2. Install and Configure Hermes

Goal of this section: get Hermes running and talking to an LLM. No Matrix yet.

### 2.1 One-shot install

```bash
curl -fsSL https://raw.githubusercontent.com/NousResearch/hermes-agent/main/scripts/install.sh | bash
```

The install script downloads Python 3.11, creates a venv, installs dependencies, and symlinks the `hermes` command into `~/.local/bin/hermes`. Once it finishes, open a new terminal, or run `source ~/.zshrc` (bash users: `~/.bashrc`), so the `hermes` command lands on your PATH.

If install goes sideways, check the [Hermes installation docs](https://hermes-agent.nousresearch.com/docs/getting-started/installation) or [GitHub Issues](https://github.com/NousResearch/Hermes-Agent/issues).

### 2.2 Verify the install

```bash
hermes --version   # expected: Hermes Agent v0.11.x
```

### 2.3 Wire up an LLM (DeepSeek as example)

```bash
hermes setup
```

The wizard asks you to paste an API key (`sk-xxxx` format, grab it from the [DeepSeek console](https://platform.deepseek.com/api_keys)) and writes it to `~/.hermes/.env`.

For other providers, `hermes --help` has the per-provider usage.

### 2.4 Optional: set a default model

```bash
hermes model
```

Called with no argument it drops into an interactive menu. The listed models are the ones Hermes's registry currently knows about (at the moment: `deepseek-reasoner` and `deepseek-chat`), plus an **Enter custom model name** option that lets you type any model ID by hand — useful whenever DeepSeek ships a new model (say `deepseek-v4`) before the registry catches up.

![hermes model interactive menu, arrow pointing to Enter custom model name](images/hermes-model-menu.png)

For specific model IDs, defer to DeepSeek's [models & pricing page](https://api-docs.deepseek.com/quick_start/pricing) — Hermes doesn't maintain an allowlist; whatever you type gets passed straight to the DeepSeek API.

That wraps up §2. If you want to sanity-check the LLM wiring before touching Matrix, just run `hermes agent` — the splash screen below (Tools / Skills listed, `deepseek-reasoner` in the lower-left, input prompt at the bottom) means the Hermes + LLM leg of the chain is working, and you're ready to move on to §3 to plug in Matrix.

![hermes agent splash screen: Hermes + LLM ready](images/hermes-agent-ready.png)

---

## 3. Log Hermes Into Matrix

Hermes is installed. Next we get it to log in to your Matrix server as a regular user.

### 3.1 Register a Matrix account for Hermes in Robrix

This step is just "register a regular account on your Matrix server and pick a name for Hermes to use." The easiest way is to do it from inside Robrix:

![Robrix's Create Account screen with `http://127.0.0.1:8128` filled in as the Homeserver URL (local Palpo case)](images/robrix-create-account-palpo.png)

| Your Matrix server | How to register |
| --- | --- |
| Local Palpo | In Robrix, set the homeserver to `http://127.0.0.1:8128` and use the registration page to create a new account |
| matrix.org | Register through Robrix or [Element Web](https://app.element.io) |
| Self-hosted Synapse | Admin API, or whatever registration page your server provides |

Write down the **username** and **password**.

> **If you're on local Palpo, please also read §3.2 — a gotcha in one sentence: the address you use to connect to Palpo and the address that shows up in your account suffix may not be the same one.** This doesn't affect public-domain deployments.

### 3.2 The local-Palpo gotcha: connection address ≠ account suffix

This gotcha isn't a Hermes quirk — it's a consequence of how Matrix IDs are designed — but because the Hermes config asks you for both **the homeserver URL** and **your full user_id**, you need to know these two values can legitimately differ.

Conceptually:

- **Homeserver URL** = the URL you use to connect to Palpo, e.g. `http://127.0.0.1:8128`
- **Account suffix** (Matrix calls this `server_name`) = the string after the colon in your Matrix ID — the "identity" Palpo declares for itself in its config at startup

Three common cases:

| Palpo's `server_name` | The URL clients use to reach Palpo | The resulting account | Same address? |
| --- | --- | --- | --- |
| `127.0.0.1:8128` | `http://127.0.0.1:8128` | `@hermes-bot:127.0.0.1:8128` | ✓ yes |
| `192.168.1.28:8128` (LAN IP) | `http://127.0.0.1:8128` | `@hermes-bot:192.168.1.28:8128` | ✗ **no** |
| `matrix.example.com` (domain) | `https://matrix.example.com` | `@hermes-bot:matrix.example.com` | ✓ yes |

**The second case is the one that trips people up** — you can reach Palpo on `127.0.0.1` and register successfully there, but the account that gets created actually has the LAN IP in its suffix. So when you configure Hermes next:

- For the homeserver URL → use `http://127.0.0.1:8128` (the one your machine can reach)
- For the user ID → use `@hermes-bot:192.168.1.28:8128` (what the account actually is)

The two addresses being **deliberately different** is normal here.

> Not sure what your account ID is? After logging into Robrix, head to Profile / Settings — the `@xxx:yyy` shown there is the full ID.

![Full User ID shown in Robrix Settings](images/robrix-user-id.png)

### 3.3 Run the Matrix config wizard

Back to the command line:

```bash
hermes gateway setup
```

Pick Matrix. The wizard first prints a block of background info about the Matrix integration (including commands for fetching an access token via Element or `curl`), then asks you two things:

- **Homeserver URL**: your Matrix server address. For local Palpo: `http://127.0.0.1:8128`. For other setups, use the domain of your Matrix service.
- **Access token**: paste your access token; if you'd rather log in with username + password, leave it empty

If you **left the token empty and chose password login**, the wizard ends here with just the homeserver written to `~/.hermes/.env`. You fill in the remaining bits (username, password, allowlist, etc.) by editing `~/.hermes/.env` yourself. A minimum working config for local Palpo looks like:

```bash
# ~/.hermes/.env
MATRIX_HOMESERVER=http://127.0.0.1:8128
MATRIX_USER_ID=@hermes-bot:192.168.1.28:8128       # see §3.2 for the server_name suffix
MATRIX_PASSWORD=your-bot-password
MATRIX_ALLOWED_USERS=@your-personal-account:192.168.1.28:8128
```

> **Using matrix.org?** Swap the homeserver for `https://matrix.org` and the user_id for `@hermes-bot:matrix.org`; everything else stays the same.

For the rest of the knobs (require @mention, enable encryption, primary rooms, etc.), the [Hermes Matrix docs](https://hermes-agent.nousresearch.com/docs/user-guide/messaging/matrix) have the full list.

### 3.4 Start the gateway — and a small detour about the Matrix library

To start:

```bash
hermes gateway
```

**A successful start looks like this** — the log should contain:

```
┌─────────────────────────────────────────────────────────┐
│           ⚕ Hermes Gateway Starting...                 │
├─────────────────────────────────────────────────────────┤
│  Messaging platforms + cron scheduler                    │
│  Press Ctrl+C to stop                                   │
└─────────────────────────────────────────────────────────┘

load: 2.11  cmd: python3.11 46130 waiting 0.43u 0.09s
```

**The first time, you'll most likely hit this warning**:

```
WARNING Matrix: mautrix not installed. Run: pip install 'mautrix[encryption]'
```

Which means the Python library the Matrix adapter needs isn't installed yet. Hermes's venv is built with uv (no pip by default), so install it like this:

```bash
uv pip install --python ~/.hermes/hermes-agent/venv/bin/python 'mautrix[encryption]'
```

Re-run `hermes gateway` and you should see the success log above.

> **Can't install the encryption variant?** `mautrix[encryption]` has a transitive dependency called `python-olm` that currently fails to build on some systems (notably newer macOS + CMake 4+). If you hit that, fall back to the plaintext variant to unblock yourself:
>
> ```bash
> uv pip install --python ~/.hermes/hermes-agent/venv/bin/python mautrix
> ```
>
> This one always installs. The cost: Hermes won't see messages in encrypted rooms. This is an upstream mautrix-python dependency issue — [mautrix/python](https://github.com/mautrix/python/issues) is the better place to track it. The plaintext variant doesn't stop you from getting the rest of the flow working — just test in a non-encrypted public room (covered in the next section).

### 3.5 A common pitfall

Worth calling out up front before you test from Robrix.

**Always put your own account into** `MATRIX_ALLOWED_USERS`

By default Hermes only responds to people on its allowlist. If **you**, messaging Hermes from your personal account, aren't in `MATRIX_ALLOWED_USERS`, Hermes silently ignores you.

That's what the wizard's "Allowed users" prompt populates. By hand in `.env`:

```
MATRIX_ALLOWED_USERS=@your-personal-account:192.168.1.28:8128
```

---

## 4. Test From Robrix

1. Log in to Robrix with **your own account** (not the bot's)
2. Search for the Matrix ID you just configured for the Hermes agent
3. Make sure you type the bot's full ID, e.g. `@hermes-bot:192.168.1.28:8128`
4. The bot auto-accepts the invite and appears in the member list within a few seconds
5. Send a message
6. Wait for the LLM reply (a few seconds to tens of seconds)

If you get a reply back, Robrix and Hermes are talking end-to-end.

![Chatting with Hermes Agent from Robrix](images/hermes-agent-reply.png)

---

## 5. Where to Look When Things Go Wrong

The table below groups issues by where they originate. Most problems aren't on the Robrix side, so the "where to fix" column is the important one.

| Symptom | Where the issue is | Where to fix |
| --- | --- | --- |
| Install script hangs, `curl \| bash` fails early | Hermes install | Hermes installation docs |
| Start fails with `mautrix not installed` / `No adapter available` | Matrix library missing | The `uv pip install` line in §3.4 |
| `python-olm` won't install, CMake errors, missing libolm | mautrix upstream dependency | [mautrix/python](https://github.com/mautrix/python/issues); fall back to the plaintext variant (§3.4) |
| Start fails with LLM auth error, unknown model, insufficient balance | LLM provider side | Your provider's console |
| Start fails with `Matrix: connection refused` | Matrix server isn't reachable | Confirm Palpo / your homeserver is running and the address is correct |
| Start fails with `Matrix: login failed: M_FORBIDDEN` | Wrong account or password | Double-check §3.2 — make sure the `server_name` suffix in your user_id matches the one Palpo declares |
| Bot receives messages but never replies (no errors in log) | Missing from allowlist | §3.5 — add your user_id to `MATRIX_ALLOWED_USERS` |
| Bot doesn't receive messages in DMs | Encryption lib missing, plaintext mode can't decrypt | §3.4 — test in a non-encrypted room, or install the encryption variant of mautrix |
| Robrix can't find the bot account | Registration didn't succeed | Log in as the bot from Element Web to confirm the account actually exists |
| Other weird Hermes behavior (CLI crashes, odd gateway state, tool-call mess) | Hermes itself | [Hermes official docs](https://hermes-agent.nousresearch.com/docs/) / GitHub Issues |

> **Rough rule of thumb**: anything that shows up "during Hermes startup" or "in Hermes's logs" — check on Hermes's side first. Anything at the "Robrix sees / doesn't see a message" layer — come back to this guide.

---

## 6. Further Reading

- **Hermes official docs:** [hermes-agent.nousresearch.com/docs](https://hermes-agent.nousresearch.com/docs/)
- **Hermes Matrix adapter reference:** [messaging/matrix](https://hermes-agent.nousresearch.com/docs/user-guide/messaging/matrix) — full environment variable list, Session Model, advanced encryption, proactive messages, etc.
- **Hermes GitHub:** [github.com/NousResearch/Hermes-Agent](https://github.com/NousResearch/Hermes-Agent)
- **Palpo deployment guide:** [01-deploying-palpo-and-octos.md](../robrix-with-palpo-and-octos/01-deploying-palpo-and-octos.md)
- **OpenClaw companion guide:** [01-deploying-openclaw-with-matrix.md](../robrix-with-openclaw/01-deploying-openclaw-with-matrix.md) — OpenClaw uses the same "regular Matrix user" integration model, so the two guides' details can be cross-checked against each other
- **OpenClaw usage guide (applies to Hermes too):** [02-using-robrix-with-openclaw.md](../robrix-with-openclaw/02-using-robrix-with-openclaw.md) — from Robrix's side the chat UX is identical for any bot that logs in as a regular Matrix user (DM flow, room invites, @mention behavior), so this covers how to actually use Hermes too
- **Robrix × OpenClaw architecture:** [03-how-robrix-and-openclaw-work-together.md](../robrix-with-openclaw/03-how-robrix-and-openclaw-work-together.md) — this explains the general "AI agent as a regular Matrix client" model, which applies to Hermes too

---

*This guide was tested against Hermes Agent v0.11.0 (April 2026). Hermes iterates quickly — field names and commands may drift; when something here doesn't match what you see, trust the [Hermes official docs](https://hermes-agent.nousresearch.com/docs/).*
