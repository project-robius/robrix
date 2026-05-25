spec: task
name: "Copy Matrix Access Token For Agent Integrations"
inherits: project
tags: [matrix, auth, settings, hermes, openclaw]
---

## Intent

Hermes Agent 的 Matrix 接入文档推荐用户在 `MATRIX_ACCESS_TOKEN` 中粘贴 Matrix access token，OpenClaw 的 Matrix 插件也支持 `accessToken`。Robrix 已经能注册、登录并持有当前 Matrix session，因此在 App 内部提供一个账号设置入口，允许用户把当前登录账号的 access token 复制到剪贴板，避免再去 Element 或手写 curl 获取 token。

## Constraints

- 必须继续遵守项目级 Matrix 异步规则：UI 只能通过 `submit_async_request(MatrixRequest::*)` 请求当前 token
- 不要新增 Cargo 依赖
- 不要运行 `cargo fmt` 或 `rustfmt`
- access token 属于敏感凭据，不能写入日志、不能渲染到 Label/TextInput、不能持久化到 AppState、不能出现在 popup 文本里
- 复制失败时只能显示非敏感错误文本

## Decisions

- UI 入口放在 `Settings > Account` 的 Other actions 区域，按钮文案为 `Copy Access Token`
- 点击按钮发送 `MatrixRequest::GetAccessTokenForCopy`
- worker 从当前全局 Matrix `Client` 读取 `client.access_token()`
- worker 通过 `AccessTokenCopyAction` 回传成功或失败；成功 payload 只在同一事件周期内用于 `cx.copy_to_clipboard`
- 成功提示使用通用文案 `Access token copied to clipboard`，不显示 token 内容
- 失败分两类，由 `AccessTokenCopyError` 枚举（`NoSession` / `Unavailable`）表示；worker 没有 `AppLanguage`，故不构造用户文案，由 UI 线程按当前语言把枚举映射成本地化错误提示

## Boundaries

### Allowed Changes
- src/sliding_sync.rs
- src/settings/account_settings.rs
- resources/i18n/en.json
- resources/i18n/zh-CN.json
- specs/task-copy-access-token.spec.md
- docs/superpowers/plans/**

### Forbidden
- Cargo.toml
- src/persistence/**
- src/login/**
- src/app.rs
- docs/robrix-with-hermes/**
- docs/robrix-with-openclaw/**

### Out of Scope
- 自动写入 `~/.hermes/.env` 或 `~/.openclaw/openclaw.json`
- 生成或刷新新的 Matrix token
- token 撤销、token 管理、设备管理 UI
- 为未登录账号读取持久化 session 文件中的 token

## Acceptance Criteria

Scenario: MatrixRequest 成功取得当前 access token
  Test: test_access_token_copy_result_returns_token_when_available
  Given 当前 Matrix client 存在并且 `access_token()` 返回 `"secret-token"`
  When worker 处理 `MatrixRequest::GetAccessTokenForCopy`
  Then 产生 `AccessTokenCopyAction::Ready { access_token: "secret-token" }`
  And token 不会写入日志或持久化状态

Scenario: 当前没有 Matrix client 时返回失败
  Test: test_access_token_copy_result_fails_without_client
  Given 当前没有登录的 Matrix client
  When worker 处理 `MatrixRequest::GetAccessTokenForCopy`
  Then 产生 `AccessTokenCopyAction::Failed { reason: NoSession }`
  And 失败 payload 只携带枚举原因，不含任何 token 字段或敏感值

Scenario: Matrix client 没有 access token 时返回失败
  Test: test_access_token_copy_result_fails_without_access_token
  Given 当前 Matrix client 存在但 `access_token()` 返回 `None`
  When worker 处理 `MatrixRequest::GetAccessTokenForCopy`
  Then 产生 `AccessTokenCopyAction::Failed { reason: Unavailable }`
  And UI 按当前语言把 `Unavailable` 映射为"当前会话没有可复制的 access token"提示

Scenario: Account Settings 成功复制 token 后只显示非敏感提示
  Test: manual_test_copy_access_token_button_copies_without_displaying_token
  Given 用户已登录 Robrix
  When 用户打开 Settings > Account 并点击 `Copy Access Token`
  Then token 被写入系统剪贴板
  And UI 显示成功 toast
  And 页面上没有显示 token 文本
  And 日志中没有出现 token 文本

Scenario: Account Settings 未登录或 token 不可用时显示错误
  Test: manual_test_copy_access_token_button_error_state
  Given 当前 session 没有可用 access token
  When 用户点击 `Copy Access Token`
  Then UI 显示非敏感错误 toast
  And 不修改剪贴板内容

## Out of Scope

- Hermes/OpenClaw 配置文件自动发现或自动写入
- 创建 bot 账号流程
- Element/curl 获取 token 的文档改写
