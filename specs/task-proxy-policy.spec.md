spec: task
name: "Proxy Policy Unified Source Of Truth"
inherits: project
tags: [proxy, network, matrix, persistence]
---

## Intent

统一 Robrix2 的网络代理策略，消除 GUI 保存配置与进程继承环境变量同时生效导致的请求路径不一致。GUI 保存的 `proxy_state.json` 是唯一来源：关闭代理必须让 Robrix 自己创建的 HTTP client 显式禁用系统代理，开启代理必须通过同一套显式策略影响 Robrix 发起的 HTTP/Matrix 请求，并且本地 homeserver 不能被外部代理环境劫持。

## Decisions

- Source of truth: `proxy_state.json` / GUI 保存值优先于 shell 或系统继承的 `http_proxy`、`https_proxy`、`all_proxy`、`NO_PROXY`
- `proxy_url = null` 表示强制无代理，Robrix 创建的 reqwest / Matrix HTTP client 必须显式调用 `no_proxy()`，不依赖也不修改进程环境变量
- `proxy_url = Some(...)` 表示启用代理，Robrix 创建的 reqwest / Matrix HTTP client 必须显式设置该 proxy URL 和统一 bypass 规则，不通过环境变量传播
- Proxy bypass baseline:
  - Loopback: `localhost`、`127.0.0.1`、`::1`
  - IPv4 RFC 1918 私有网段: `10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16`
  - IPv4 link-local: `169.254.0.0/16`
  - IPv6 ULA (RFC 4193): `fc00::/7`
  - IPv6 link-local: `fe80::/10`
- Bypass 是 best-effort，目的是让 GUI 填了公网 HTTP proxy（如 `127.0.0.1:7890` 的 Clash）的同时仍能直连 LAN 上的 self-hosted homeserver/Palpo。Trade-off: 公司 VPN 将公网 homeserver 解析到 RFC 1918 时也会走直连——这是预期行为（VPN 已提供加密传输），目前不提供 GUI 例外配置
- 不硬编码具体 LAN IP；只硬编码标准化网段。如未来需要非 RFC 1918 的额外 bypass（如 CGNAT、自定义 LAN），应作为 GUI 配置项进入 `proxy_state.json`
- Matrix SDK client、homeserver discovery reqwest client、直接下载 reqwest client、updater reqwest client 必须复用同一 proxy policy helper
- TSP 使用不同 reqwest 版本，必须镜像同一 proxy policy: TLS 1.2、共享 `DEFAULT_NO_PROXY_BYPASS`、无 GUI proxy 时显式禁用 system proxy
- 显式 reqwest proxy 必须带相同 no-proxy bypass 规则；无 GUI proxy 时显式禁用 reqwest system proxy
- 加载已保存的 proxy 时，遇到当前 `validate_proxy_url` 不再支持的 scheme（如旧版 `socks5://`）必须降级为 None 并 `warning!`，不让陈旧记录在 HTTP client 构造期才以不透明错误浮出
- 不新增 Cargo 依赖，不运行 `cargo fmt`

## Boundaries

### Allowed Changes
- src/proxy_config.rs
- src/sliding_sync.rs
- src/persistence/matrix_state.rs
- src/login/login_screen.rs
- src/settings/settings_screen.rs
- src/tsp/mod.rs
- src/updater.rs
- specs/task-proxy-policy.spec.md

### Forbidden
- 不要修改 Matrix session 删除策略或 token 失效判断
- 不要重构登录、设置页的 UI 布局
- 不要添加新的 Cargo 依赖
- 不要运行 `cargo fmt` 或 `rustfmt`

## Completion Criteria

Scenario: 保存关闭代理不会修改进程环境
  Test: save_proxy_url_none_persists_direct_policy
  Level: unit
  Test Double: temp proxy_state file
  Targets: src/proxy_config.rs
  Given GUI proxy policy 为 None
  When GUI 保存 `proxy_url = null`
  Then `proxy_state.json` 保存 null
  And Robrix 不提供任何运行期写入代理环境变量的 production API
  And 强制无代理由后续 HTTP client 的显式 `no_proxy()` 实现

Scenario: 保存开启代理不会通过环境变量传播
  Test: save_proxy_url_some_persists_proxy_policy
  Level: unit
  Test Double: temp proxy_state file
  Targets: src/proxy_config.rs
  Given GUI 输入代理 "http://127.0.0.1:7890"
  When 保存代理配置
  Then `proxy_state.json` 保存该代理
  And Robrix 不提供任何运行期写入代理环境变量的 production API
  And 代理传播由后续 HTTP client 的显式 proxy builder 实现

Scenario: 显式 reqwest client 遵循无代理策略
  Test: build_policy_reqwest_client_disables_system_proxy_when_proxy_is_none
  Level: unit
  Test Double: in-memory client builder construction
  Targets: src/proxy_config.rs, src/sliding_sync.rs
  Given GUI proxy policy 为 None
  When Robrix 构建 homeserver discovery、下载、restore session 或 updater 使用的 reqwest client
  Then client builder 显式禁用 system proxy
  And 本地 homeserver 请求不会被旧环境代理污染

Scenario: 显式 reqwest proxy 包含 loopback + 私有网段 bypass
  Test: build_policy_reqwest_client_attaches_no_proxy_bypass_for_local_addresses
  Level: unit
  Test Double: reqwest proxy debug representation
  Targets: src/proxy_config.rs, src/sliding_sync.rs
  Given GUI proxy policy 为 "http://127.0.0.1:7890"
  When Robrix 构建 homeserver discovery、下载、restore session 或 updater 使用的 reqwest client
  Then 显式 proxy 使用相同代理 URL
  And no-proxy bypass 包含 localhost、127.0.0.1、::1
  And no-proxy bypass 包含 RFC 1918 网段（10.0.0.0/8、172.16.0.0/12、192.168.0.0/16）
  And no-proxy bypass 包含 link-local 与 IPv6 ULA（169.254.0.0/16、fc00::/7、fe80::/10）
  And 不包含硬编码的具体局域网 homeserver IP

Scenario: 非 Matrix reqwest 路径遵循同一代理策略
  Test: updater_http_client_disables_system_proxy_when_proxy_is_none
  Test: tsp_proxy_policy_disables_system_proxy_when_proxy_is_none
  Test: tsp_proxy_policy_attaches_no_proxy_bypass_for_local_addresses
  Level: unit
  Test Double: code path inspection and shared helper construction
  Targets: src/updater.rs, src/tsp/mod.rs
  Given GUI proxy policy 为 None
  When updater 或 TSP 创建 reqwest client
  Then updater 复用 `build_policy_reqwest_client`
  And TSP 镜像同一 policy 并显式调用 `no_proxy()`

Scenario: 无效代理 URL 被拒绝
  Test: discovery_http_client_rejects_invalid_proxy_override
  Level: unit
  Test Double: in-memory client builder construction
  Targets: src/proxy_config.rs, src/sliding_sync.rs
  Given GUI 或登录页输入代理 "ftp://proxy.invalid"
  When Robrix 构建 discovery HTTP client
  Then 构建失败并报告不支持的 proxy scheme

Scenario: 加载旧版 socks5 保存会降级为无代理
  Test: load_saved_proxy_url_ignores_legacy_socks_scheme
  Level: unit
  Test Double: temp proxy_state file with legacy socks5 entry
  Targets: src/proxy_config.rs
  Given `proxy_state.json` 中保留了旧版本写入的 `socks5://127.0.0.1:1080`
  When Robrix 启动并调用 `load_saved_proxy_url`
  Then 返回 None 而非透传 socks5 URL
  And 记录 warning 提示用户重新在 Settings 中保存支持的 scheme

Scenario: cargo build passes
  Test: cargo_build
  Level: integration
  Targets: cargo build
  Given proxy policy 统一化改动完成
  When 运行 `cargo build`
  Then 构建通过

## Out of Scope

- Palpo 服务端配置或部署修改
- auto-login PR 的 session 删除策略修改
- 代理认证 UI 的视觉调整
- 支持 PAC、WPAD 或平台系统代理发现
