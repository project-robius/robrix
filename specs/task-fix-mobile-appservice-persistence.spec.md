spec: task
name: "移动端 App Service 绑定跨强杀持久化修复"
inherits: project
tags: [bugfix, persistence, mobile, app-service, bot-settings]
estimate: 0.5d
---

## 意图

修复 issue #94（https://github.com/Project-Robius-China/robrix2/issues/94）：在 Android（以及其他移动平台）上，用户在 Settings → Labs → App Service 里填写的 BotFather User ID 和 Octos Service URL，在 force-quit + 重启之后全部丢失，必须每次重新录入。保存路径本身已经正确持久化；真正的 bug 在加载路径——`src/sliding_sync.rs::handle_load_app_state` 用一个"dock 非空"的 if-守卫把整个 `RestoreAppStateFromPersistentState` 派发动作全部挡在外面。移动端没有 dock，结果每次重启都会把加载回来的非 dock 字段（`selected_room`、`bot_settings`、`app_language`、`translation`）静默丢弃。修复方式是把加载侧守卫改为 content-aware gate：`load_app_state` 成功且返回的 `AppState` 含有有意义的非默认持久化内容时才派发恢复动作；全新安装 / 无持久化文件返回的纯默认 `AppState` 保持 no-op。

## 约束

- 保留现有的 `skip_app_state_restore_once` 后门：如果用户显式登出留下的标记文件存在，就不要恢复 app state（当前 `src/sliding_sync.rs` 中 `take_skip_app_state_restore_once` 调用点的先后顺序必须保持不变）
- 保留 `src/app.rs` 里 `AppStateAction::RestoreAppStateFromPersistentState` 匹配分支的既有语义：保留 `logged_in_actual`、通过 `remove_room_bindings_where` 剔除陈旧的房间绑定、剔除后重新持久化、派发 `MainDesktopUiAction::LoadDockFromAppState`
- 不要改动 `save_app_state` 代码路径，也不要改 `AppState` / `BotSettingsState` 的 JSON 序列化格式
- 不要改 `app_data_dir()` / `persistent_state_dir()` 的路径解析——移动端路径本身是对的，bug 纯粹是派发动作丢失
- 单元测试覆盖 serde 往返契约、restore gate 判定逻辑、Settings UI 从恢复后的 `AppState` hydrate 的判定逻辑；不碰文件 I/O，确保无副作用、可确定性执行

## 决策

- 修复位置：`src/sliding_sync.rs::handle_load_app_state`——移除原先基于 dock 非空的守卫，改为当 `load_app_state` 返回的 `AppState` 含有任何非默认持久化内容（`selected_room`、dock、bot_settings、app_language、translation）时派发 `AppStateAction::RestoreAppStateFromPersistentState(Box::new(app_state))`
- 理由：`src/app.rs:1071-1095` 的恢复匹配分支已经做了完整的 `self.app_state = *app_state.clone()` 替换，并且无条件派发 `MainDesktopUiAction::LoadDockFromAppState`。因此不能再用"dock 非空"来决定是否恢复；否则移动端的非 dock 字段仍会丢失。但对于全新安装 / 无持久化文件返回的纯默认 `AppState`，保持 no-op 更符合既有语义，也避免用一份默认值去覆盖运行中的瞬时状态
- 修改 `handle_load_app_state` 内的日志文案，从 "Loaded room panel state from app data directory. Restoring now..." 改为 "Loaded app state from persistent storage. Restoring now..."，防止后续读代码的人误以为这条路径只恢复 dock
- 回归测试：在 `src/app.rs` 已有的 `#[cfg(test)] mod tests` 模块（约从 2568 行开始）内追加一个 serde 往返单测。构造一个启用了 App Service 的 `AppState`（`bot_settings.enabled = true`、非默认的 `botfather_user_id`、非默认的 `octos_service_url`）、`saved_dock_state_home` 为空；用 `serde_json::to_string` 序列化后再反序列化，断言三个 `bot_settings` 字段全部存活
- restore gate 直接测试：在 `src/sliding_sync.rs` 的 `matrix_request_tests` 中覆盖空 dock + `bot_settings` 会恢复、空 dock + `selected_room` 会恢复、纯默认 `AppState` 不恢复
- Settings UI hydrate：`src/settings/bot_settings.rs::BotSettings` 在事件处理入口从 `Scope<AppState>` 检查 `bot_settings` 是否已恢复或是否尚未首次 hydrate；需要时调用 `sync_ui`。不要依赖移动端上滑退出时的 `Shutdown` 保存，因为强杀不保证应用收到 shutdown 事件；保存仍然发生在 Save / Check Now / toggle 这些用户动作上
- 单测名称保持与场景 `测试:` 选择器一致：`test_app_state_roundtrip_preserves_bot_settings_with_empty_dock`——把"防止哪种 bug"写进名字，未来维护者一眼能懂
- 验证层次：机械层用单测覆盖 serde 契约；端到端的 Android force-quit + 重启场景用 `Test: manual_test_*` 形式绑定，交给用户手动验收

## 边界

### 允许变更
- `src/sliding_sync.rs`——只改 `handle_load_app_state` 及其 restore gate helper / 单元测试
- `src/app.rs`——只在已有的 `#[cfg(test)] mod tests` 块里追加新单测；`app.rs` 生产代码不改
- `src/settings/bot_settings.rs`——只改 Settings UI 从 `Scope<AppState>` hydrate 的判定和相关单测；不改保存路径语义
- `issues/009-mobile-appservice-binding-not-persisted.md`——修复落地后补写 "Fix Applied" 段

### 禁止
- 不要改 `src/persistence/app_state.rs`（save/load 本身没问题）
- 不要改 `src/persistence/matrix_state.rs`（`persistent_state_dir`、`app_data_dir` 解析）
- 不要改 `src/app.rs` 中 `RestoreAppStateFromPersistentState` 匹配分支的生产代码
- 不要改 `src/settings/bot_settings.rs` 的保存路径语义（保存路径正确；只允许补 Settings UI hydrate）
- 不要改 `AppState` / `BotSettingsState` 的字段布局、`#[serde]` 属性、默认值——这会影响用户设备上已有 JSON 的向后兼容性
- 不要给 `Cargo.toml` 加 dev-dependency；serde_json 已经可用，不需要 mocking 框架
- 不要新增对外公开 API（没有新的 `pub fn`，没有新的 `pub struct`）
- 不要跑 `cargo fmt`
- 不要在用户手动验收通过之前 commit 或者创建 PR

## 验收标准

场景: 空 dock 的 AppState 序列化往返保留 bot_settings 所有字段
  测试: test_app_state_roundtrip_preserves_bot_settings_with_empty_dock
  层级: unit
  命中: app_state_serde_roundtrip
  假设 构造一个 `AppState`，其 `bot_settings.enabled` 为 `true`
  并且 `bot_settings.botfather_user_id` 为 `"@octosbot:example.com"`
  并且 `bot_settings.octos_service_url` 为 `"http://192.168.5.12:8010"`
  并且 `saved_dock_state_home.open_rooms` 为空
  并且 `saved_dock_state_home.dock_items` 为空
  当 通过 `serde_json::to_string` 序列化后再 `serde_json::from_str` 反序列化回来
  那么 反序列化后的 `bot_settings.enabled` 等于 `true`
  并且 反序列化后的 `bot_settings.botfather_user_id` 等于 `"@octosbot:example.com"`
  并且 反序列化后的 `bot_settings.octos_service_url` 等于 `"http://192.168.5.12:8010"`

场景: 空 dock 且含 bot_settings 时 restore gate 允许恢复
  测试: test_should_restore_loaded_app_state_with_bot_settings_and_empty_dock
  层级: unit
  命中: handle_load_app_state_restore_gate
  假设 构造一个 `AppState`，其 `bot_settings` 非默认
  并且 `saved_dock_state_home.open_rooms` 为空
  并且 `saved_dock_state_home.dock_items` 为空
  当 调用 `should_restore_loaded_app_state`
  那么 返回 `true`

场景: 空 dock 且含 selected_room 时 restore gate 允许恢复
  测试: test_should_restore_loaded_app_state_with_selected_room_and_empty_dock
  层级: unit
  命中: handle_load_app_state_restore_gate
  假设 构造一个 `AppState`，其 `selected_room` 非空
  并且 `saved_dock_state_home.open_rooms` 为空
  并且 `saved_dock_state_home.dock_items` 为空
  当 调用 `should_restore_loaded_app_state`
  那么 返回 `true`

场景: 纯默认 AppState 时 restore gate 不派发恢复
  测试: test_should_not_restore_loaded_default_app_state
  层级: unit
  命中: handle_load_app_state_fresh_install
  假设 构造一个 `AppState::default()`
  当 调用 `should_restore_loaded_app_state`
  那么 返回 `false`

场景: Settings UI 在 AppState 恢复后重新 hydrate App Service 表单
  测试: test_bot_settings_scope_sync_detects_restored_state
  层级: unit
  命中: bot_settings_scope_hydration
  假设 `BotSettings` 上次同步的是默认 `BotSettingsState`
  并且 `Scope<AppState>` 中的 `bot_settings` 已恢复为非默认 App Service 配置
  当 判断是否需要从 `AppState` 同步 UI
  那么 返回 `true`

场景: Settings UI 第一次处理事件时即使 AppState 是默认值也执行 hydrate
  测试: test_bot_settings_scope_sync_runs_before_first_hydration
  层级: unit
  命中: bot_settings_scope_hydration
  假设 `BotSettings` 还没有完成首次同步
  并且 `Scope<AppState>` 中的 `bot_settings` 是默认值
  当 判断是否需要从 `AppState` 同步 UI
  那么 返回 `true`

场景: Settings UI 已同步且 AppState 未变化时不重复 hydrate
  测试: test_bot_settings_scope_sync_ignores_unchanged_state
  层级: unit
  命中: bot_settings_scope_hydration
  假设 `BotSettings` 已经完成首次同步
  并且 上次同步的 `BotSettingsState` 与 `Scope<AppState>` 中的 `bot_settings` 相同
  当 判断是否需要从 `AppState` 同步 UI
  那么 返回 `false`

场景: 移动端 force-quit + 重启后 App Service 绑定得到恢复
  测试: manual_test_mobile_app_service_persists_across_force_quit
  层级: manual
  命中: handle_load_app_state_mobile
  假设 用户在 Android 上已登录
  并且 用户打开 Settings → Labs → App Service
  并且 用户启用 App Service 并填写 BotFather user ID `"@octosbot:192.168.5.12:8128"` 与 Octos service URL `"http://192.168.5.12:8010"`
  并且 用户点击 Save 看到成功提示
  当 用户执行 `adb shell am force-stop dev.makepad.robrix` 强杀 robrix2
  并且 用户重新启动 robrix2 并回到 Settings → Labs → App Service
  那么 BotFather user ID 输入框显示 `"@octosbot:192.168.5.12:8128"`
  并且 Octos service URL 输入框显示 `"http://192.168.5.12:8010"`
  并且 点击 "Check Now" 返回 Reachable

场景: 显式登出后再次登录仍然跳过 app state 恢复
  测试: manual_test_skip_app_state_restore_once_marker_is_honored
  层级: manual
  命中: handle_load_app_state_skip_marker
  假设 用户存在一份已持久化的 `latest_app_state.json` 且包含非默认的 bot_settings 绑定
  并且 该用户目录下存在 `skip_app_state_restore_once` 标记文件
  当 应用启动并对该用户执行 `handle_load_app_state`
  那么 `load_app_state` 不会被调用且不会派发 `RestoreAppStateFromPersistentState`
  并且 标记文件被消耗，下一次启动时恢复正常进行

场景: 全新安装没有持久化文件时不派发恢复且使用默认 bot_settings
  测试: manual_test_fresh_install_no_restore_dispatched
  层级: manual
  命中: handle_load_app_state_fresh_install
  假设 当前登录用户没有任何 `latest_app_state.json`
  当 应用启动并执行 `handle_load_app_state`
  那么 `load_app_state` 返回默认 `AppState`
  并且 bot_settings 等于 `BotSettingsState::default()`，其中 `enabled` 为 `false`
  并且 不会弹出任何错误提示

场景: App state 文件损坏时回退到默认值并备份原文件
  测试: manual_test_corrupt_app_state_fallback_preserves_original
  层级: manual
  命中: load_app_state_corrupt_fallback
  假设 `latest_app_state.json` 存在但内容为非法 JSON
  当 应用启动并执行 `handle_load_app_state`
  那么 `load_app_state` 把原文件重命名为 `latest_app_state.json.bak`
  并且 返回 `AppState::default()`
  并且 恢复派发不会传播任何陈旧的 bot_settings 值
  并且 用户仍然可以正常进入 App

场景: 桌面端有完整 dock 状态时继续恢复 dock 以及 bot_settings
  测试: manual_test_desktop_dock_restoration_regression_guard
  层级: manual
  命中: handle_load_app_state_desktop_regression
  假设 用户在 macOS/Linux/Windows 上已有持久化的 `AppState` 且 `saved_dock_state_home.dock_items` 非空
  并且 bot_settings 已启用且含有自定义 BotFather user ID
  当 应用启动并执行 `handle_load_app_state`
  那么 `RestoreAppStateFromPersistentState` 被精确派发一次
  并且 dock 布局通过 `MainDesktopUiAction::LoadDockFromAppState` 得到恢复
  并且 bot_settings 反映了持久化的绑定

场景: 恢复派发之后陈旧的房间绑定仍然会被剔除
  测试: manual_test_stale_room_bindings_pruned_on_restore
  层级: manual
  命中: restore_handler_prune_stale_bindings
  假设 持久化的 `AppState` 中 `bot_settings.room_bindings` 包含了用户已经不在的房间条目
  当 应用启动且恢复匹配分支处理 `RestoreAppStateFromPersistentState`
  那么 陈旧的房间绑定通过 `remove_room_bindings_where` 被剔除
  并且 剔除后的状态通过 `persistence::save_app_state` 重新持久化

## 排除范围

- 在 App Service 设置页加 "Last saved: <timestamp>" 标签（issue #94 提到的防御性 UX，单独追踪为独立任务）
- iOS 端的专门验收测试（iOS 与 Android 共用同一套持久化抽象；移动端修复天然覆盖两端；iOS 专属验证单独立项）
- 把 `bot_settings` 从 `AppState` 里独立成单独的文件
- 修改 JSON schema、文件布局、或加 schema-version 元数据
- 重写 dock 状态的保存/恢复语义
- Android 上 `ProjectDirs` 解析之外的多进程/多设备存储隔离
