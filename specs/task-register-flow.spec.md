spec: task
name: "双模账号注册（OIDC + UIAA），对齐 Element Desktop"
inherits: project
tags: [feature, register, login, matrix, uiaa, oidc, mas, ui]
estimate: 5w
---

## Intent

在 robrix2 中实现完整的 Matrix 账号注册流程，行为上对齐 Element Desktop 1.12.x：当服务器声明支持 MSC3861 / MAS OAuth 时，把注册委托给系统浏览器经 OIDC（使用 matrix-sdk 的 `oidc_auth` API）加 loopback HTTP 回调完成；当服务器只支持传统 Interactive Authentication（UIAA）时，robrix2 在本地渲染一个向导式 wizard 引导用户逐 stage 完成 UIAA，对无法本地渲染的 stage（reCAPTCHA、UIAA 中的 SSO 阶段、未知类型）用 UIAA 规范里的 `fallback/web` URL 作为逃生舱。

目标同时满足两点：代码**功能隔离**——所有注册相关代码放进新的 `src/register/` 目录；用户**习惯兼容**——`login_screen.rs` 的 "Sign up here" 入口按钮文字、位置、样式完全不变。协议行为和 wizard 结构以 element-web 的 `Registration.tsx` + `InteractiveAuthEntryComponents.tsx` 为参考蓝本。

## Constraints

- 双模分路必须由**运行时服务器能力发现**驱动（`.well-known/matrix/client` + `/_matrix/client/versions` + `/_matrix/client/v3/login` + 空 body `POST /register` 探测），禁止对服务器做硬编码判断
- 禁止在客户端硬编码 identity provider ID：所有 SSO 按钮和 OIDC 端点都来自服务器响应，不得出现像 `"oidc-google"` 这样的客户端字面量
- 所有新增 Matrix I/O 都走 `submit_async_request(MatrixRequest::*)` — 禁止为注册相关网络调用裸开 `tokio::spawn`
- UIAA 错误判定必须用 `uiaa_info.auth_error.kind` 的结构化字段来区分"stage 做错了"和"推进下一 stage"，禁止对错误消息做字符串匹配
- `src/login/login_screen.rs` 的 "Sign up here" 按钮文字、位置、视觉样式保持不变；只改它的点击行为（从原来调用 `set_signup_mode(true)` 改为派发 `LoginAction::NavigateToRegister`）
- 所有收集用户输入的 UIAA stage widget（协议勾选、注册令牌输入、邮件等待、短信验证码）必须用 Makepad 2.0 `script_mod!` DSL 加 Widget derive 实现
- UIAA stage 的分派和推进由 UI 层的 `UiaaController` widget 协调，不得把状态机埋进 `sliding_sync.rs`，确保 stage 渲染和等待用户输入都在 UI 层发生
- OIDC 回调的 loopback 端口必须每次尝试动态分配（不得用固定端口），避免端口冲突并允许用户连续尝试多次注册而无需重启
- 注册时提交的设备名默认为 `"Robrix2 on {OS}"`，其中 `{OS}` 来自 `std::env::consts::OS`，后续用户可在设置里改名
- 批准新增一项依赖：`zxcvbn`（v3.x 系列），用于密码强度估算；此处的批准记录即为正式批准，不再需要单独改 `project.spec.md`
- 除 `zxcvbn` 外不增加任何新依赖；系统浏览器调用复用 `sliding_sync.rs` 已引入的 `robius_open` crate
- 本 spec 不包含任何移动端深链（如 `robrix://callback`）URL scheme 注册；loopback HTTP 足以覆盖本 spec 所限定的桌面端目标
- 注册完成后必须走已有的自动登录 / `finalize_authenticated_client` / 会话持久化路径 — 禁止为注册成功另开一条鉴权落地代码路径

## Decisions

### 范围与对齐对象

- 目标：对齐 Element Desktop 1.12.x 的完整行为，实现方式是**双模**客户端 — 既不是单模全原生，也不是单模全浏览器
- 参考物：element-web 的 `Registration.tsx` / `RegistrationForm.tsx` / `InteractiveAuthEntryComponents.tsx`（协议与 wizard 结构）；本地安装的 Element Desktop 1.12.15（路径 `/Applications/Element.app/Contents/Resources/webapp.asar`）已实证包含全部 7 种 UIAA auth type 字符串以及 MSC3861 / OIDC 相关字符串
- 现有 robrix2 的起点：`src/sliding_sync.rs` 第 435–492 行已存在 `LoginRequest::Register` 分支，但只认 `m.login.dummy`，其它 flow 一律 bail；本 spec 用完整的双模分派器替换那段代码
- 先验参考（不是代码平移来源）：https://github.com/project-robius/robrix/pull/579 — 借鉴 `RegisterScreen` 布局思路和 `RegistrationForm` 字段顺序；**不**沿用 PR #579 里硬编码的 Google-only SSO、min-8 字符密码启发式、以及字符串匹配判 UIAA 错误

### 模块结构

- 新建目录 `src/register/`，且只有 6 个文件：
  - `mod.rs` — 导出和模块级文档
  - `register_screen.rs` — 注册主屏：ServerPicker、能力探测、分派到 OIDC 或 UIAA 分支
  - `oidc.rs` — OIDC / MAS OAuth 分支：loopback HTTP、通过 `robius_open` 打开浏览器、回调处理、code 换 token
  - `uiaa.rs` — UIAA 分支：`UiaaController` + 6 个 stage widget（`DummyStage` / `TermsStage` / `RegistrationTokenStage` / `EmailStage` / `MsisdnStage` / `FallbackStage`）
  - `register_status_modal.rs` — 进度模态，两个分支共用
  - `validation.rs` — localpart 校验器、URL 校验器、zxcvbn 包装
- `src/lib.rs` 追加 `pub mod register;`
- `src/app.rs` 新增对 `LoginAction::NavigateToRegister` 和 `RegisterAction::*` 的分派以完成屏幕切换
- `src/login/login_screen.rs` 删除 signup-mode 分支（confirm_password 字段渲染、`set_signup_mode` 函数、Sign Up 按钮内部的注册提交逻辑，共约 80 行），并把 "Sign up here" 按钮的 click 改为派发 `LoginAction::NavigateToRegister`；按钮文字、位置、样式保持不变
- `src/sliding_sync.rs` 新增 `MatrixRequest` 变体：`DiscoverHomeserverCapabilities`、`RegisterViaOidc`、`RegisterViaUiaa`、`ContinueUiaa`、`RequestRegisterEmailToken`、`RequestRegisterMsisdnToken`、`CheckUsernameAvailable`；删除旧的 `LoginRequest::Register` 分支

### 服务器能力发现（两分支共用前置）

- 进入 `RegisterScreen` 并在用户确认 homeserver URL 后派发 `DiscoverHomeserverCapabilities`：
  - GET `/.well-known/matrix/client`（宽松模式：失败时回退到用户输入的原始 URL，对齐 Element 的 AutoDiscoveryUtils）
  - GET `/_matrix/client/versions`（存活性检查 + feature flag）
  - GET `/_matrix/client/v3/login`（枚举顶层 flow：`m.login.password`、`m.login.sso` 带 identity_providers 等）
  - POST `/_matrix/client/v3/register` 空 body（预期 401 → 解析 UIAA `flows` 和 `params`；若返回 403 `M_FORBIDDEN` 则表示服务器禁用注册）
  - 检查 `unstable_features["org.matrix.msc3861"]` 和/或 `/auth_issuer` OIDC 发现端点来判定 MAS 支持
- 返回 `HsCapabilities { is_mas_native_oidc: bool, registration_enabled: bool, sso_providers: Vec<IdentityProvider>, uiaa_probe: Option<UiaaInfo> }`
- `RegisterScreen` 按 `is_mas_native_oidc` 分派：
  - `true` → 渲染 OIDC 卡片，显示 "Continue in browser" 按钮
  - `false && registration_enabled` → 渲染 `RegistrationForm`（走 UIAA 分支）
  - `false && !registration_enabled` → 渲染 "This server doesn't allow registration" 和 "Log in instead" 按钮

### OIDC / MAS 分支（`src/register/oidc.rs`）

- 使用 matrix-sdk 的 OIDC API：`client.matrix_auth_oidc().url_for_oidc_login(OidcRegistrationData)`
- 打开浏览器**之前**先在 `http://localhost:<动态端口>/callback` 启一个短生命周期的 loopback HTTP server
- 用 `robius_open::Uri::new(&authorization_url).open()` 启动系统浏览器（沿用 `src/sliding_sync.rs:5969` 处已有的用法）
- 处理 loopback 回调：解析 `code` 和 `state`，对比出站 `state` 参数校验，再通过 matrix-sdk 把 code 换成 tokens
- 用户侧 UX：进度模态文案从 "Opening browser…" 切到 "Waiting for completion…"，带一个可见的 Cancel 按钮（点击关掉 loopback 并返回 RegisterScreen）
- 取消逻辑：关闭 loopback server，关闭模态，恢复 "Continue in browser" 按钮可用
- 超时（默认 5 分钟，未来可配置）：在超时后报错文案 inline 显示并返回 RegisterScreen
- 成功：派发 `RegisterAction::OidcSuccess(Client)` → `finalize_authenticated_client()` → `LoginAction::LoginSuccess`
- 对**非 MAS 但 `/v3/login` 公布了 `m.login.sso`** 的服务器：顶层 SSO 登录沿用 `src/sliding_sync.rs` 已有的 `SpawnSSOServer` 机制（不重复实现）；注册侧 SSO 使用同一机制，只是语义上带 registration intent

### UIAA 分支（`src/register/uiaa.rs`）

- `RegistrationForm` 结构体一次性采集：username、password、confirm_password、可选 email、可选带区号 phone、可选 registration_token、device_name 默认
- email 和 phone 字段是否显示由 `uiaa_probe.flows` 决定：只要**任一** flow 要 `m.login.email.identity` 或 `m.login.msisdn`，就显示对应字段并标必填（与 Element 规则一致）
- SSO 按钮（由 `/v3/login` 发现）显示在密码表单旁；点击走已有的 `SpawnSSOServer` 路径
- 点击 Submit 时，`RegistrationForm` 依次执行：
  - 客户端校验：username 非空且满足 localpart 正则、password 长度 ≥1 且 zxcvbn 分数 ≥3、confirm_password 与 password 一致、email（若填了）格式合法、phone（若填了）格式合法
  - 异步查重：username 失焦或输入暂停 500ms 后派发 `CheckUsernameAvailable`，调 `GET /_matrix/client/v3/register/available?username=<value>`，UI 渲染 available / taken / invalid / checking
  - 派发 `RegisterViaUiaa(RegistrationForm)`；sliding_sync 侧发出第一次真正的 `/register`，`initial_device_display_name` 已设置
- UIAA 循环：
  - 返回 200 → 成功，拿到 `access_token`，走 `finalize_authenticated_client()`
  - 返回 401 且带 `auth_error` → 当前 stage 被拒，在 stage widget 上 inline 显示错误，`stage_index` 不动
  - 返回 401 不带 `auth_error` → 正常推进，根据选定的 flow 取下一个 stage，渲染对应 widget
- Flow 选择（首次 UIAA 进入时、渲染第一个 stage widget 之前）：
  - 对每个候选 flow 打分：本地可渲染的 stage 加分；要 fallback URL 的 stage 减分；要 3pid 但用户未填对应字段的减分
  - 选分数最高的；同分选 stage 数最少的
- Stage widget（每个 ≤150 行，全部放在同一个 `uiaa.rs` 文件里）：
  - `DummyStage` — 无 UI，自动提交 `AuthData::Dummy { session }`
  - `TermsStage` — 每条 `params.policies` 一个勾选框（策略名是可点击链接，点开后用 `robius_open` 打开策略 URL）；Continue 按钮在全部勾完后才可用
  - `RegistrationTokenStage` — 单行文本输入；提交时派发 `AuthData::RegistrationToken { session, token }`
  - `EmailStage` — 进入时用表单里已采集的 email 调 `RequestRegisterEmailToken` 拿 `sid`；显示 "Verification email sent to {email}" + Continue 按钮；点 Continue 重新 POST `/register`，`auth.threepid_creds = {sid, client_secret}`；不做轮询
  - `MsisdnStage` — 进入时用表单里的 phone 调 `RequestRegisterMsisdnToken`；显示短信验证码输入框；提交派发 `AuthData::Msisdn { session, threepid_creds }`
  - `FallbackStage` — 兜底 `m.login.recaptcha`、`m.login.sso`（UIA 阶段中的 SSO）、所有未识别类型：通过 `robius_open` 打开 `GET {homeserver}/_matrix/client/v3/auth/{stage_type}/fallback/web?session={session}`；显示 "Complete verification in browser, then click Continue"；Continue 重新 POST `/register`，body 不变、session 不变、不带新 auth 字段

### 错误处理

- 统一 `RegisterError` 枚举，11 个变体：`UserInUse`、`InvalidUsername`、`WeakPassword`、`ThreepidInUse`、`Forbidden`、`RateLimited`、`MasNotAvailable`、`OidcCallbackTimeout`、`NetworkError`、`UnsupportedFallback`、`Other(String)`
- Matrix `ErrorKind` 到 `RegisterError` 的映射放在 `uiaa.rs::map_matrix_error()` 里，使用结构化字段（禁止字符串匹配）
- 屏幕级错误（HS 不可达、不允许注册）inline 显示在 RegisterScreen 顶部
- Stage 级错误（token 无效、3pid 已占用）inline 显示在当前 stage widget 上；已输入的字段值保持不变

### 用户名查重

- Debounce：输入框失焦或输入暂停 500ms 后触发
- 端点：`GET /_matrix/client/v3/register/available?username=<value>`
- UI 状态：`Idle`、`Checking`、`Available`、`Taken`、`Invalid`、`NetworkError`
- 视觉：用户名输入框旁边显示小图标加文字状态

### 密码强度

- 使用 `zxcvbn` crate v3.x（新依赖，本 spec 批准）
- 输入变化时计算得分（debounce 150ms）
- UI：0-4 分强度条；0-2 分时显示本地化提示
- 得分 <3 禁用 Submit 按钮（对齐 Element 的 `PASSWORD_MIN_SCORE`）
- 仅作用于 UIAA 分支 — OIDC 分支的密码在 MAS 网页里输入，不归 robrix2 管

### 注册成功后（两分支共用）

- 两条分支都以 `finalize_authenticated_client(client, client_session, &user_id, /* is_add_account */ false)` 收尾；该函数已有，复用不改
- 不提供注册时手填设备名 UI；名字以 `"Robrix2 on {OS}"` 预填，用户可在 Settings 里改名
- 本 spec 不包含 E2EE 密钥备份 UI（归属 `src/verification/` 的独立关注点）

### 分阶段实施（spec 本身是完整且单一的；实现分 5 个 PR 落地）

- **阶段 1 — 骨架 + 能力发现**：建 `src/register/` 目录；`RegisterScreen` 带 ServerPicker；`DiscoverHomeserverCapabilities` 实现；`login_screen.rs` 删 signup mode；`NavigateToRegister` 连线。可感知价值：用户能选 HS 并看到 "MAS / UIAA / 禁用"
- **阶段 2 — OIDC 分支**：`oidc.rs` 完整（loopback + 浏览器 + 回调 → token）。可感知价值：matrix.org 新用户能完成注册
- **阶段 3 — UIAA 核心 + 本地 stage**：`uiaa.rs` 加 `UiaaController`、`DummyStage`、`TermsStage`、`RegistrationTokenStage`；密码 + confirm 输入。可感知价值：Palpo / 简单自建 Synapse 能密码注册
- **阶段 4 — 3pid + fallback**：`EmailStage`、`MsisdnStage`、`FallbackStage`；多 provider SSO 按钮从 `/v3/login` 动态渲染。可感知价值：覆盖完整 Synapse 部署
- **阶段 5 — 打磨**：zxcvbn 强度条、用户名实时查重、错误文案 i18n、设备名 OS 检测、UI 细节。可感知价值：Element Desktop 级观感

只有阶段 1 的 plan 会和本 spec 一起交付；阶段 2-5 的 plan 在前序阶段合并后再各自撰写。

## Boundaries

### Allowed Changes

- 创建：`src/register/mod.rs`、`src/register/register_screen.rs`、`src/register/oidc.rs`、`src/register/uiaa.rs`、`src/register/register_status_modal.rs`、`src/register/validation.rs`
- 修改：`src/lib.rs`（追加 `pub mod register;`）
- 修改：`src/app.rs`（派发 `LoginAction::NavigateToRegister` 和 `RegisterAction::*` 来完成屏幕切换与 finalize 调用）
- 修改：`src/login/login_screen.rs`（删除 signup-mode 渲染和处理代码约 80 行；把 "Sign up here" 点击改为派发 `LoginAction::NavigateToRegister`；按钮文字/位置/样式保持不变）
- 修改：`src/sliding_sync.rs`（新增能力发现、OIDC 注册、UIAA 注册、stage 推进、3pid token 请求、用户名查重等 `MatrixRequest::*` 变体；删除旧的 `LoginRequest::Register` 分支；`finalize_authenticated_client` 复用不改）
- 修改：`Cargo.toml`（在 `[dependencies]` 下追加 `zxcvbn = "3"`）
- 修改：i18n 资源文件，新增注册相关文案
- 修改：`DESIGN.md`（可选，阶段 5 批量更新：在模块组织一节加入 `src/register/`）

### Forbidden

- 除了 `zxcvbn = "3"`，禁止对 `Cargo.toml` 做任何其它依赖变更
- 禁止修改 `finalize_authenticated_client()` 的签名或它的持久化 / 拉起 sync 行为
- 禁止在客户端加 SSO identity provider 名（如 `"oidc-google"`）作为常量；provider 全部来自服务器响应
- 本 spec 内禁止任何深链 URL scheme（如 `robrix://`）或移动端特有回调的实现
- 禁止对任何文件执行 `cargo fmt`
- 禁止把 `LoginAction::LoginBySSOSuccess` 复用作注册成功载体；新增 `RegisterAction::OidcSuccess(Client)` 变体
- 禁止引入任何内嵌 webview 依赖（`wry`、`webview2` 等）；所有需要浏览器的 stage 全部通过 `robius_open` 调起系统浏览器
- 禁止依赖对 Matrix 错误消息做字符串匹配；UIAA 错误分类必须使用 `auth_error.kind` 结构化字段
- 禁止以严格模式实现 `.well-known`（即不得强制 DNS 可解或坚持 JSON 合法）；使用 Element 的宽松模式
- `.well-known` 请求超时不得超过 3 秒，超过后回退到原始 URL（对齐 Element 行为）
- 禁止为注册引入新的 `cpu_worker::CpuJob` 变体 — 注册 I/O 全是网络，不是 CPU
- 禁止让用户手动复制粘贴任何回调 code；loopback HTTP server 必须直接接收回调

## Out of Scope

- 移动端（iOS / Android / OpenHarmony）的回调处理和自定义 URL scheme 注册 — 本 spec 只覆盖桌面端 loopback
- 注册成功后的 E2EE 密钥备份 / 恢复密钥设置 UI
- 注册后的账号资料编辑（头像、显示名）— 注册成功账号进入主界面后，资料为空，沿用已有 Settings UI
- 多账号注册（已登录状态下再注册另一个账号）— 本 spec 不覆盖 add-account 注册；只保留 add-account 登录的现有行为
- 账号找回 / 密码重置 — 独立特性，不属于本 spec
- Identity Server 绑定 3pid — robrix2 不配置 identity server，完全依赖 homeserver 自己的 `register/email/requestToken` 和 `register/msisdn/requestToken` 端点
- reCAPTCHA 原生渲染 — 明确委托给系统浏览器经 fallback URL，永远不原生实现
- 注册漏斗的 telemetry / 分析
- Wizard 布局的 A/B 测试
- `login_screen.rs` 的 signup-mode 被**删除**，不保留为"经典模式"切换
- 注册时通过表单自定义设备名 — 设备名固定为 `"Robrix2 on {OS}"`，用户后续在 Settings 改名

## Completion Criteria

5 个实施阶段各有可观测里程碑；以下 21 条场景覆盖整个 spec。场景名前带 `[Phase X]` 标签标明所属阶段。

Scenario: 登录屏的入口按钮保持不变
  Test: manual_test_signup_entry_button_layout
  Given the user is on the login screen
  When the user visually inspects the "Sign up here" button
  Then the button text, position, icon, and style match the current pre-change appearance
  And clicking it navigates to RegisterScreen (not toggling signup mode on login screen)

Scenario: [Phase 1] RegisterScreen 显示 homeserver 选择并完成能力探测
  Test: manual_test_phase1_hs_discovery
  Given the user clicks "Sign up here" on the login screen
  When the user enters a homeserver URL (or leaves empty for matrix.org default)
  And clicks Next
  Then RegisterScreen calls DiscoverHomeserverCapabilities
  And the screen indicates whether the server uses MAS OAuth, UIAA, or disallows registration

Scenario: [Phase 1] 不允许注册的服务器显示友好错误
  Test: manual_test_phase1_registration_disabled
  Given a homeserver that returns M_FORBIDDEN on empty /register probe
  When the user completes HS discovery
  Then RegisterScreen shows "This server doesn't allow registration"
  And a "Log in instead" button returns to login screen

Scenario: [Phase 2] 现代 homeserver 把注册路由到系统浏览器
  Test: manual_test_phase2_oidc_happy_path
  Given a homeserver advertising MSC3861 / MAS OAuth (e.g. matrix.org)
  When the user clicks "Continue in browser"
  Then robrix2 opens the MAS registration page in the system browser
  And robrix2 shows a "Waiting for completion" modal with a Cancel button
  And after the user completes registration in the browser
  Then the loopback callback fires, access_token is obtained
  And robrix2 transitions to the main app logged in as the new account

Scenario: [Phase 2] OIDC 取消按钮关闭 loopback server
  Test: manual_test_phase2_oidc_cancel
  Level: integration
  Targets: src/register/oidc.rs cancel path; loopback http server lifecycle
  Given the "Waiting for completion" modal is visible
  When the user clicks Cancel
  Then the loopback HTTP server is shut down
  And the modal closes
  And RegisterScreen returns to capability-discovery state

Scenario: [Phase 2] OIDC 超时 inline 报错
  Test: manual_test_phase2_oidc_timeout
  Level: integration
  Targets: src/register/oidc.rs timeout path; callback wait budget
  Given the OIDC modal has been waiting for 5 minutes without a callback
  When the timeout elapses
  Then robrix2 closes the loopback server
  And shows an error inline: "Browser registration timed out. Please try again."

Scenario: [Phase 3] 传统 homeserver 路由到 UIAA wizard
  Test: manual_test_phase3_uiaa_entry
  Given a homeserver that does not advertise MAS but allows password registration
  When the user completes capability discovery
  Then RegisterScreen renders RegistrationForm with username, password, confirm_password fields
  And SSO provider buttons matching /v3/login response (if any)

Scenario: [Phase 3] 仅 dummy 的 flow 无可见 stage UI 即完成
  Test: manual_test_phase3_dummy_only
  Given a homeserver whose flows contain just [m.login.dummy]
  When the user submits valid username and password
  Then UiaaController auto-submits the dummy stage with the session
  And registration succeeds without showing any stage widget
  And the user is logged in

Scenario: [Phase 3] Terms flow 显示协议勾选框
  Test: manual_test_phase3_terms_stage
  Given a homeserver flow contains m.login.terms with two policies
  When the user reaches the terms stage
  Then a checkbox is rendered per policy
  And each policy name is a clickable link opening the policy URL in system browser
  And the Continue button is disabled until all checkboxes are checked

Scenario: [Phase 3] Registration token flow 显示 token 输入
  Test: manual_test_phase3_token_stage
  Given a homeserver flow requires m.login.registration_token
  When the user reaches the token stage
  Then a text input labeled "Registration token" is rendered
  And entering an invalid token and submitting shows an inline error without clearing the field
  And entering a valid token advances to the next stage (or completes)

Scenario: [Phase 4] Email stage 使用表单里预采集的 email
  Test: manual_test_phase4_email_stage
  Given a homeserver flow requires m.login.email.identity and the RegistrationForm had email collected upfront
  When the user reaches the email stage
  Then robrix2 calls register/email/requestToken with the form email
  And the stage shows "Verification email sent to {email}. Click the link, then press Continue"
  And pressing Continue without clicking the email link resubmits /register and shows an inline "Not yet verified" message on 401
  And pressing Continue after clicking the link advances to the next stage

Scenario: [Phase 4] Msisdn stage 显示短信验证码输入
  Test: manual_test_phase4_msisdn_stage
  Given a homeserver flow requires m.login.msisdn and the form had phone collected upfront
  When the user reaches the msisdn stage
  Then robrix2 calls register/msisdn/requestToken
  And the stage shows a text input for the SMS code
  And entering the wrong code and submitting shows inline error without clearing the field
  And entering the correct code advances the flow

Scenario: [Phase 4] reCAPTCHA stage 通过系统浏览器走 fallback
  Test: manual_test_phase4_recaptcha_fallback
  Given a homeserver flow requires m.login.recaptcha
  When the user reaches the recaptcha stage
  Then robrix2 opens {homeserver}/_matrix/client/v3/auth/m.login.recaptcha/fallback/web?session={session} in the system browser
  And the stage UI shows "Complete verification in your browser, then click Continue"
  And pressing Continue resubmits /register without a new auth field
  And a successful fallback advances the flow

Scenario: [Phase 4] 未知 stage 类型走系统浏览器兜底
  Test: manual_test_phase4_unknown_stage_fallback
  Given a homeserver flow contains a stage type not in {dummy, terms, token, email, msisdn, recaptcha, sso}
  When the user reaches the unknown stage
  Then robrix2 opens the /auth/{type}/fallback/web URL in the system browser
  And the Continue button works identically to the recaptcha case

Scenario: [Phase 4] 多 provider SSO 按钮从服务器动态渲染
  Test: manual_test_phase4_multi_provider_sso
  Given a homeserver's /v3/login response contains 3 identity providers (Google, GitHub, Apple)
  When the user is on RegisterScreen's form
  Then 3 SSO buttons render, each with the name from the server response
  And clicking any button launches the existing SSO flow with the corresponding provider_id

Scenario: [Phase 5] 密码强度条使用 zxcvbn
  Test: manual_test_phase5_password_strength
  Given the user types "password" into the password field
  Then a strength bar shows score ≤2 and a hint like "Too weak: avoid common passwords"
  And the Submit button is disabled
  When the user types "correcthorsebatterystaple"
  Then the strength bar shows score ≥3
  And the Submit button is enabled

Scenario: [Phase 5] 用户名查重有 debounce
  Test: manual_test_phase5_username_availability
  Given the user types "alice" in the username field
  When 500ms elapses without further typing
  Then robrix2 calls /register/available
  And the result (Available / Taken / Invalid) shows next to the field
  And typing more characters cancels the pending check and schedules a new one after another 500ms

Scenario: [Phase 5] 设备名默认按 OS 填入
  Test: manual_test_phase5_device_name_default
  Given registration succeeds via either OIDC or UIAA branch
  When the account is logged in
  Then the new device is named "Robrix2 on macos" (or appropriate OS string)

Scenario: 登录屏的密码登录行为不变（回归守卫）
  Test: manual_test_regression_password_login
  Given the login screen is shown after the register feature is merged
  When the user enters existing credentials and clicks Sign In
  Then password login completes as it did before this spec

Scenario: 已有 SSO 登录流程不变（回归守卫）
  Test: manual_test_regression_sso_login
  Given the user is on the login screen
  When the user clicks any SSO provider button
  Then the existing SSO login flow runs unchanged (no interference from register code)

Scenario: 注册账号的会话持久化
  Test: manual_test_session_persistence_after_register
  Given the user successfully registers via OIDC or UIAA branch
  When the user closes robrix2 and reopens it
  Then the user is still logged in as the registered account without needing to sign in again
