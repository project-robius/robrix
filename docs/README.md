# Robrix Documentation

Welcome to the Robrix documentation. Choose a guide based on your use case.

---

## Robrix Only

For users who want to use Robrix as a standalone Matrix client, connecting to matrix.org or any existing homeserver:

| Guide | Goal |
|-------|------|
| [Getting Started with Robrix](robrix/getting-started-with-robrix.md) | **Install Robrix and start chatting.** Download or build Robrix, connect to a Matrix server, register an account, and join rooms. |

> Chinese: [Robrix 快速开始](robrix/getting-started-with-robrix-zh.md)

---

## Robrix + Palpo + Octos (AI Bot System)

For users who want to deploy a complete AI chat system — running your own Matrix homeserver with AI bot capabilities, then using Robrix to chat with AI bots.

**Default path is the 3-step lightweight mode** — Palpo + PostgreSQL run in Docker, Octos runs as a native host process, and you only edit `.env` between two scripts:

```sh
./setup.sh          # clones Palpo source + downloads Octos bundle
$EDITOR .env        # set DEEPSEEK_API_KEY
./start.sh          # Palpo (Docker) + Octos (native)
```

| Guide | Goal |
|-------|------|
| [1. Deploying Palpo and Octos](robrix-with-palpo-and-octos/01-deploying-palpo-and-octos.md) | **Get Palpo homeserver and Octos AI bot running (3-step lightweight mode).** `./setup.sh` downloads the upstream Octos bundle and clones Palpo source; edit `.env`; `./start.sh` brings up Palpo + PostgreSQL in Docker and Octos as a native process. |
| [2. Using Robrix with Palpo and Octos](robrix-with-palpo-and-octos/02-using-robrix-with-palpo-and-octos.md) | **Use Robrix to chat with AI bots on your Palpo server.** Step-by-step with screenshots: log in, create rooms, invite bots, have conversations, and manage bots through the BotFather system. |
| [3. How Robrix, Palpo, and Octos Work Together](robrix-with-palpo-and-octos/03-how-robrix-palpo-octos-work-together.md) | **Understand the Application Service mechanism.** Learn how Octos registers as a Matrix App Service on Palpo, how messages flow from Robrix through Palpo to the AI bot, and how the BotFather system manages multiple bots. |
| [4. Federation with Palpo](robrix-with-palpo-and-octos/04-federation-with-palpo.md) | **Enable cross-server communication.** Configure Palpo for Matrix federation so users on different servers can chat with each other and access your AI bots. |

> Chinese:
> [1. 部署 Palpo 和 Octos](robrix-with-palpo-and-octos/01-deploying-palpo-and-octos-zh.md) ·
> [2. 在 Robrix 上使用 Palpo 和 Octos](robrix-with-palpo-and-octos/02-using-robrix-with-palpo-and-octos-zh.md) ·
> [3. Robrix、Palpo、Octos 协作原理](robrix-with-palpo-and-octos/03-how-robrix-palpo-octos-work-together-zh.md) ·
> [4. Palpo 联邦功能](robrix-with-palpo-and-octos/04-federation-with-palpo-zh.md)

---

## Robrix + OpenClaw (AI Agent Framework)

For users who want to connect OpenClaw AI agents to Matrix, then use Robrix to chat with them:

| Guide | Goal |
|-------|------|
| [1. Deploying OpenClaw with Matrix](robrix-with-openclaw/01-deploying-openclaw-with-matrix.md) | **Get OpenClaw connected to a Matrix homeserver.** Create a bot account, configure the Matrix channel plugin, and verify the connection so Robrix can chat with OpenClaw agents. |
| [2. Using Robrix with OpenClaw](robrix-with-openclaw/02-using-robrix-with-openclaw.md) | **Use Robrix to chat with OpenClaw agents.** Start conversations via DM or rooms, understand feature compatibility, and learn the differences from the Octos workflow. |
| [3. How Robrix and OpenClaw Work Together](robrix-with-openclaw/03-how-robrix-and-openclaw-work-together.md) | **Understand the client-based integration model.** Learn how OpenClaw connects to Matrix as a regular client (vs. Octos's Appservice model), how messages flow, and how E2EE works. |

> Chinese:
> [1. 部署 OpenClaw + Matrix](robrix-with-openclaw/01-deploying-openclaw-with-matrix-zh.md) ·
> [2. 在 Robrix 上使用 OpenClaw](robrix-with-openclaw/02-using-robrix-with-openclaw-zh.md) ·
> [3. Robrix 与 OpenClaw 协作原理](robrix-with-openclaw/03-how-robrix-and-openclaw-work-together-zh.md)

---

## Robrix + Hermes (AI Agent Framework)

For users who want to connect [Hermes Agent](https://github.com/NousResearch/Hermes-Agent) to Matrix, then use Robrix to chat with it:

| Guide | Goal |
|-------|------|
| [1. Deploying Hermes with Matrix](robrix-with-hermes/01-deploying-hermes-with-matrix.md) | **Get Hermes Agent connected to a Matrix homeserver.** Install Hermes, wire up an LLM, log the bot in as a regular Matrix user, and verify end-to-end chat from Robrix. Also covers the local-Palpo `server_name` gotcha and the `mautrix[encryption]` install snag. |

> **Usage and architecture:** Hermes and OpenClaw share the same "AI agent as a regular Matrix client" integration model, so from Robrix's side the usage UX is identical and the architecture is the same. For those topics see the OpenClaw guides: [2. Using Robrix with OpenClaw](robrix-with-openclaw/02-using-robrix-with-openclaw.md) and [3. How Robrix and OpenClaw Work Together](robrix-with-openclaw/03-how-robrix-and-openclaw-work-together.md).

> Chinese:
> [1. 部署 Hermes + Matrix](robrix-with-hermes/01-deploying-hermes-with-matrix-zh.md) · 使用方式与架构原理同 OpenClaw：[2. 在 Robrix 上使用 OpenClaw](robrix-with-openclaw/02-using-robrix-with-openclaw-zh.md) · [3. Robrix 与 OpenClaw 协作原理](robrix-with-openclaw/03-how-robrix-and-openclaw-work-together-zh.md)

---

## Palpo and Octos Deployment Files

The [`palpo-and-octos-deploy/`](../palpo-and-octos-deploy/) directory (at the repository root) holds the runnable lightweight-mode deployment — see its [README](../palpo-and-octos-deploy/README.md) for the 3-step Quickstart.

```
palpo-and-octos-deploy/
├── README.md                    # 3-step Quickstart, supported platforms, disk budget
├── setup.sh                     # Auto-detects platform, clones Palpo, installs Octos bundle
├── start.sh                     # Lifecycle: start / stop / restart / status / logs
├── compose.yml                  # Palpo + PostgreSQL (Octos runs natively, not in compose)
├── palpo.Dockerfile             # Palpo source build (unified cross-platform)
├── palpo.toml                   # Palpo homeserver config
├── .env.example                 # Environment variables template (DEEPSEEK_API_KEY, etc.)
├── appservices/
│   └── octos-registration.yaml  # Appservice registration (Palpo ↔ Octos, via host.docker.internal)
├── config/
│   ├── botfather.json           # Bot profile and LLM settings
│   └── octos.json               # Octos global settings
├── octos-bin/                   # Native Octos binary (installed by setup.sh, gitignored)
├── repos/                       # Palpo source (cloned by setup.sh, gitignored)
├── logs/                        # Octos stdout/stderr (gitignored)
└── data/                        # Runtime data — Postgres + Palpo media (gitignored)
```

For always-on Octos with systemd / launchd / frpc tunnel, see Octos upstream docs: <https://github.com/octos-org/octos>.
