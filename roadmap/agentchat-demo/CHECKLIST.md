# Demo 验收 Checklist(robrix2 × agent-chat × 本地 Palpo)

分三层:**A 自动**(跑 `preflight.sh` 一键验)、**B 一次性安装**(按序执行)、**C 运行期人工**(脚本测不了的行为)。
你的三个担忧已尽量自动化:① Palpo :8128 登录/MXID 域 → A;② `!mkgroup` 成功 → A(API 层) + C(房间层);③ approve 只认发起人 → C。

---

## A. 自动检查 —— 跑一条命令

```bash
cd roadmap/agentchat-demo
# 建议:先填好 agent-chat/.env,启动 Palpo;backend 起来后再跑可顺带验 !mkgroup
AC_DIR=/Users/zhangalex/Work/Projects/consult/agent-chat ./preflight.sh
```

它会逐项打勾,覆盖:

- [ ] **工具链**:node / tmux / curl / `agent-spec`(版本 + `parse` + `lint --min-score`)
- [ ] **Palpo CS-API 可达** `http://127.0.0.1:8128/_matrix/client/versions`(担忧①:端口对不对)
- [ ] **well-known** 指向 `127.0.0.1:8128`(担忧①:MXID 域是否一致)
- [ ] **`.env` 必填项**都已填(非占位符):`API_TOKEN` / `MATRIX_BRIDGE_SECRET` / `MATRIX_BOT_PASSWORD` / `MATRIX_AGENT_PASSWORD_SECRET` / `MATRIX_HOMESERVER` / `MATRIX_SERVER_NAME`
- [ ] **bot 真实登录** `@agent-bridge` 成功且 MXID 域 = `127.0.0.1:8128`(担忧①:登录真能过)
- [ ] **skill 已链接**到 `~/.claude/skills/issue-workflow` 和 `~/.codex/skills/issue-workflow`
- [ ] **`MATRIX_BRIDGE_SECRET` 真能建群**:用真实 API `POST /api/groups`(建一个 `_preflight_*` 临时群再删)→ 直接证明 `!mkgroup` 不会 403(担忧②:API 层)

> `preflight.sh` 退出码 0 = 必需项全过。带 `[opt]` 的是可选项(如 backend 没起时跳过建群测试),不会让它失败。

---

## B. 一次性安装(按顺序)

- [ ] **B1. 起 Palpo**(`palpo-and-octos-deploy/`,compose 映射 `8128:8008`,开放注册已开)。
      ✅ 你的 OrbStack 里已在跑(`palpo-and-octos-deploy-palpo-1`,`0.0.0.0:8128->8008`)。
- [ ] **B2. 配 `.env`**:把 `agent-chat.env.demo` 内容并入 `agent-chat/.env`,填 3 个**你自己现编**的值:
      - `API_TOKEN` = 任意非空串(后端没它会拒绝启动;**不是 LLM key**,本地随便填,如 `dev-token-change-me`)
      - `MATRIX_BOT_PASSWORD` = 给 `@agent-bridge` 账号设的密码(自定)
      - `MATRIX_AGENT_PASSWORD_SECRET` = 任意长随机串(`openssl rand -hex 24`);agent 密码由它派生
      - `MATRIX_BRIDGE_SECRET` 本地可**留空**(后端为空时跳过校验,127.0.0.1 又豁免 bearer)
      - `MATRIX_REG_TOKEN` **留空**;`MATRIX_HOMESERVER/_SERVER_NAME` 已是 `127.0.0.1:8128`,不用改
- [ ] **B2b. 预建账号**:`AC_DIR=... node register-accounts.mjs`(或由 start-demo.sh 自动执行)。
      你的 Palpo 用 `m.login.dummy` 注册(已实测),但 agent-chat 的 bridge 只会用 token flow 自注册——
      所以**先预建好 4 个账号**(dummy flow),bridge 之后只「登录」不「注册」。幂等,已实测可用。
- [ ] **B3. 起后端三件套**(`start-demo.sh` 会做,且用 `/health` 轮询替代盲 sleep):
      backend(`node backend-v2.js`)→ bridge(`node bridge-matrix.js`,首启自动注册 `@agent-bridge`)→ push-relay(`PUSH_RELAY_MODE=local node push-relay.js`)。
- [ ] **B4. link skill**:`./link-skill.sh`(把 `issue-workflow` 链进 `~/.claude/skills` + `~/.codex/skills`)。
- [ ] **B5. 起 4 个 agent**(`start-demo.sh` 会做)—— 3 个 Claude + 1 个 **Codex** 终审:
      `agentchat up-v1 wf_coordinator|wf_implementer|wf_reviewer claude …`,外加
      `agentchat up-v1 wf_final_reviewer codex --project <repo> --project-mode symlink --allow-shared-workspace --fresh`(codex 那个会多花点时间抓 resume-id)。
- [ ] **B6. 复跑 `preflight.sh`** —— 此时 backend 已起,确认"建群测试"那项变 ✓。

> B2b–B5 一键:`DEMO_REPO=<目标仓> ./start-demo.sh`(它会先 `register-accounts.mjs` 预建账号,再起服务+起 agent)。

---

## C. 运行期人工验收(脚本测不了的行为)

在 robrix2 里操作:

- [ ] **C1. 登录**:robrix2 登 `http://127.0.0.1:8128`,你的账号能登入、能看时间线。
      (担忧①:Palpo 在 OrbStack docker 已实测 CS-API 可达 + well-known=`:8128`;这步只是最终在客户端确认登录往返。)
- [ ] **C2. 建群**:邀 `@agent-bridge` 进任意房 → 发 `!mkgroup demoboard wf_coordinator wf_implementer wf_reviewer wf_final_reviewer`
      → bridge 新建 `demoboard` 房并邀请你 + 4 个 agent;**~30s 内自动 join**。(担忧②房间层最终确认)
- [ ] **C3. 建 issue**:在 demoboard 发 `@wf_coordinator /create-issue 登录闪退 | 点登录按钮崩溃`
      → wf_coordinator 在群里回帖:已写 `issues/NNN-*.md`、`agent-spec lint` 得分、请 `approve`。
- [ ] **C4. approve 门禁**(担忧③):
      - 用**你(发起人)**发 `approve` → wf_coordinator 继续(plan→implement→review)。✓
      - (可选反例)让**另一个人/账号**发 `approve` → wf_coordinator **应忽略**。这条是 skill 提示词行为,
        靠观察验证;若它误接受,需在 `issue-workflow/SKILL.md` 的 wf_coordinator `approve` 段加强 `from` 校验。
- [ ] **C5. 全链路可见**:群里能看到 wf_coordinator → wf_implementer → wf_reviewer(Claude 对抗审查)
      → **wf_final_reviewer(Codex 独立终审,自己重跑 build)** 的往返,最终 wf_coordinator 回帖
      `Issue NNN complete ✅ (reviewer + Codex final gate)`(或打回重做)。
- [ ] **C5b. Codex 终审确实独立**:wf_final_reviewer 的回帖里应有 `Final verdict (Codex)`、独立 `Build re-run: cargo check exit N`、
      以及 `Context: peek=… · capture=…` 痕迹;它应在 wf_reviewer **approve 之后**才介入,且能挑出第一审遗漏(或确认无遗漏)。
- [ ] **C6. /status**:发 `@wf_coordinator /status` → 回帖各 issue 状态表。

---

## 出问题时看哪里
- `agent-chat/.demo-logs/{backend,bridge,relay}.log`(start-demo.sh 写)
- agent 终端:`agentchat attach <name>` 或 tmux,确认它看到 `[NOTIFICATION]` 并调了 `check_inbox()`
- 建群 403 → `MATRIX_BRIDGE_SECRET` 两端不一致(preflight §6 会直接报)
- agent 不 join → trust gate;demo 用 `MATRIX_TRUST_MODE=audit`
- wf_coordinator post 不进群 → 群名没学对;确认它从触发消息的 `group` 字段取名(见 SKILL.md)
