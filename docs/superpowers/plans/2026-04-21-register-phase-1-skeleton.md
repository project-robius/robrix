# Register 阶段 1 实施计划：骨架 + HS 能力发现

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 `src/register/` 模块的骨架（4 个文件），实现 homeserver 能力发现，让用户从 login 屏点击 "Sign up here" 后进入注册屏选服务器并看到 "MAS OAuth / UIAA / 不允许注册" 三种状态。本阶段**不**实现任何真正的注册动作。

**Architecture:** 独立 `src/register/` 模块，包含 `mod.rs`、`register_screen.rs`、`register_status_modal.rs`、`validation.rs`。`RegisterScreen` widget 主导 UI；新增 `MatrixRequest::DiscoverHomeserverCapabilities` 在后台异步探测 `.well-known/matrix/client` + `/_matrix/client/versions` + `/_matrix/client/v3/login` + 空 body `POST /register`。结果通过 `RegisterAction::CapabilitiesDiscovered(HsCapabilities)` 回到 UI。`login_screen.rs` 删除现有 signup mode 并把 "Sign up here" 按钮的点击改为派发 `LoginAction::NavigateToRegister`。

**Tech Stack:** Rust + Makepad 2.0 DSL (`script_mod!`)，`matrix-sdk`（已引入），`url = "2.5"`（已引入），`reqwest`（通过 matrix-sdk transitive dep 已引入），`robius_open`（已引入，但本阶段不使用），`tokio`（已引入）。

**依赖的 spec：** `specs/task-register-flow.spec.md` ，阶段 1 对应 "骨架 + 能力发现" 子章节。

---

## File Structure

### Create

| 文件 | 职责 | 预计 LOC |
|---|---|---|
| `src/register/mod.rs` | 模块入口 + `RegisterAction` / `HsCapabilities` / `RegisterMode` 数据类型 + `script_mod(vm)` 聚合函数 | ~120 |
| `src/register/register_screen.rs` | `RegisterScreen` widget 及其 `script_mod!` DSL；ServerPicker、能力状态显示 | ~250 |
| `src/register/register_status_modal.rs` | 进度/状态模态（Phase 1 仅骨架） | ~80 |
| `src/register/validation.rs` | URL 归一化 + 校验 + 对应单元测试 | ~80 |

### Modify

| 文件 | 改动 |
|---|---|
| `src/lib.rs` | 追加 `pub mod register;`；`script_mod` 注册点调用 `register::script_mod(vm)` |
| `src/login/login_screen.rs` | 删除 signup mode 相关代码（confirm_password DSL 字段、`set_signup_mode`、Sign Up 内部注册逻辑），改 "Sign up here" 按钮 click 派发 `LoginAction::NavigateToRegister` |
| `src/login/login_screen.rs` (enum) | `LoginAction` 新增 `NavigateToRegister` 变体 |
| `src/app.rs` | 处理 `LoginAction::NavigateToRegister` / `RegisterAction::NavigateToLogin` 实现登录屏 ↔ 注册屏切换 |
| `src/sliding_sync.rs` (L695 附近) | `MatrixRequest` 新增 `DiscoverHomeserverCapabilities { url: String }` 变体 |
| `src/sliding_sync.rs` (worker loop) | 实现能力发现 handler：调 matrix-sdk + 直接 HTTP probe 四条端点 |

### Out of Phase 1 (留给阶段 2+)

- `src/register/oidc.rs`（阶段 2）
- `src/register/uiaa.rs`（阶段 3）
- 实际触发注册：本阶段仅显示能力状态，**不**发起真正的 /register

---

## Prerequisite: 现有代码的关键位置（执行者速查）

- `MatrixRequest` enum 位置：`src/sliding_sync.rs:695`
- `LoginRequest` enum + `RegisterAccount` struct：`src/sliding_sync.rs:1245-1270`
- `LoginAction` enum：`src/login/login_screen.rs:1223`
- 现有 signup mode 相关 click handler：`src/login/login_screen.rs` 中搜索 `set_signup_mode`
- Matrix worker loop 的 match 点：`src/sliding_sync.rs:1288`（`match request { ... }`）
- `login/mod.rs` 中的 `script_mod` 聚合模式：就是我们要在 `register/mod.rs` 复制的结构

---

## Task 1: 模块骨架 + lib.rs 注册

**Files:**
- Create: `src/register/mod.rs`
- Create: `src/register/validation.rs`
- Create: `src/register/register_screen.rs`（本 task 只做占位 stub）
- Create: `src/register/register_status_modal.rs`（占位 stub）
- Modify: `src/lib.rs`（追加 `pub mod register;`）

- [ ] **Step 1：创建 `src/register/mod.rs`（最小骨架）**

```rust
//! Account registration feature.
//!
//! Covers the dual-mode register flow (OIDC for MAS-delegated servers,
//! UIAA wizard for legacy servers). See `specs/task-register-flow.spec.md`.

use makepad_widgets::ScriptVm;

pub mod register_screen;
pub mod register_status_modal;
pub mod validation;

pub fn script_mod(vm: &mut ScriptVm) {
    register_status_modal::script_mod(vm);
    register_screen::script_mod(vm);
}
```

- [ ] **Step 2：创建 `src/register/validation.rs`（占位）**

```rust
//! URL / localpart / password validators for registration.

// Functions are added in Task 2.
```

- [ ] **Step 3：创建 `src/register/register_screen.rs`（最小 stub）**

```rust
//! RegisterScreen widget: homeserver picker + capability display.
//!
//! The full wizard body is added in later phases; Phase 1 only wires
//! server selection and shows the MAS/UIAA/Disabled three-state result.

use makepad_widgets::*;

script_mod! {
    use makepad_widgets::base::*;
    use makepad_widgets::theme_desktop_dark::*;

    pub RegisterScreen := {{RegisterScreen}} View {
        width: Fill,
        height: Fill,
        show_bg: true,
        draw_bg: { color: #x1F2124 }

        // TODO: Task 5 fills in this body.
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterScreen {
    #[deref] view: View,
}

impl Widget for RegisterScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
```

- [ ] **Step 4：创建 `src/register/register_status_modal.rs`（最小 stub）**

```rust
//! Status modal shared by both OIDC and UIAA branches.
//!
//! Phase 1 scaffolds the widget; full wiring comes in Phases 2-4.

use makepad_widgets::*;

script_mod! {
    use makepad_widgets::base::*;
    use makepad_widgets::theme_desktop_dark::*;

    pub RegisterStatusModal := {{RegisterStatusModal}} Modal {
        // TODO: Phase 2 wires title + status text + cancel button.
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterStatusModal {
    #[deref] modal: Modal,
}

impl Widget for RegisterStatusModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.modal.handle_event(cx, event, scope);
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.modal.draw_walk(cx, scope, walk)
    }
}
```

- [ ] **Step 5：修改 `src/lib.rs`，追加 `pub mod register;`**

定位：在 `pub mod login;` 后面（让 login 和 register 视觉上相邻）。

编辑示意：

```rust
pub mod login;
pub mod register;  // NEW
pub mod logout;
```

同时，在该文件的 `script_mod` 聚合函数里加入 `register::script_mod(vm)`。搜索 `login::script_mod(vm)` 所在位置，紧跟其后加一行：

```rust
pub fn script_mod(vm: &mut ScriptVm) {
    // ... existing lines ...
    login::script_mod(vm);
    register::script_mod(vm);  // NEW
    // ... other lines ...
}
```

> **注意**：具体 `script_mod` 聚合函数在 `src/lib.rs` 里的位置可能略有差异；执行者 grep `login::script_mod(vm)` 定位后插入即可。

- [ ] **Step 6：验证 `cargo build` 通过**

```bash
cargo build 2>&1 | tail -20
```

预期：无 error。有 `unused import` 或 `dead_code` warning 是正常的（本阶段是骨架）。

- [ ] **Step 7：commit**

```bash
git add src/register/ src/lib.rs
git commit -m "feat(register): scaffold src/register/ module

Add empty register module with mod.rs, register_screen.rs,
register_status_modal.rs, validation.rs. Register the module
in src/lib.rs and wire the script_mod aggregator per the
login/mod.rs pattern.

Part of specs/task-register-flow.spec.md Phase 1."
```

---

## Task 2: URL 归一化 + 单元测试

**Files:**
- Modify: `src/register/validation.rs`

**TDD 顺序：先写测试，再写实现。**

- [ ] **Step 1：在 `src/register/validation.rs` 写测试（用 `#[cfg(test)]` 模块）**

```rust
//! URL / localpart / password validators for registration.

use url::Url;

/// Errors returned by homeserver URL normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeserverUrlError {
    /// Input was empty or whitespace only.
    Empty,
    /// Scheme is neither `http` nor `https`.
    UnsupportedScheme(String),
    /// URL could not be parsed.
    Invalid,
}

/// Normalize a user-entered homeserver URL.
///
/// - Bare hostname (e.g. `matrix.org`) becomes `https://matrix.org`.
/// - Explicit `http(s)://` schemes are kept as-is.
/// - Any non-`http(s)` scheme is rejected.
/// - Trailing `/` is stripped.
/// - Empty string is rejected.
pub fn normalize_homeserver_url(input: &str) -> Result<Url, HomeserverUrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(HomeserverUrlError::Empty);
    }

    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let mut url = Url::parse(&with_scheme).map_err(|_| HomeserverUrlError::Invalid)?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(HomeserverUrlError::UnsupportedScheme(other.to_string())),
    }

    // Strip trailing slash for canonical form (Url keeps path = "/" by default;
    // set to "" so discovery appends cleanly).
    if url.path() == "/" {
        url.set_path("");
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_hostname_gets_https() {
        let url = normalize_homeserver_url("matrix.org").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn explicit_https_is_kept() {
        let url = normalize_homeserver_url("https://alvin.meldry.com").unwrap();
        assert_eq!(url.as_str(), "https://alvin.meldry.com");
    }

    #[test]
    fn explicit_http_is_kept() {
        let url = normalize_homeserver_url("http://127.0.0.1:8128").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:8128");
    }

    #[test]
    fn trailing_slash_is_stripped() {
        let url = normalize_homeserver_url("https://matrix.org/").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn whitespace_is_trimmed() {
        let url = normalize_homeserver_url("  matrix.org  ").unwrap();
        assert_eq!(url.as_str(), "https://matrix.org");
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(
            normalize_homeserver_url(""),
            Err(HomeserverUrlError::Empty),
        );
        assert_eq!(
            normalize_homeserver_url("   "),
            Err(HomeserverUrlError::Empty),
        );
    }

    #[test]
    fn non_http_scheme_is_rejected() {
        let result = normalize_homeserver_url("ftp://example.com");
        assert!(matches!(result, Err(HomeserverUrlError::UnsupportedScheme(ref s)) if s == "ftp"));
    }

    #[test]
    fn malformed_url_is_rejected() {
        let result = normalize_homeserver_url("http://  /not a url");
        assert!(matches!(result, Err(HomeserverUrlError::Invalid)));
    }
}
```

- [ ] **Step 2：跑测试验证**

```bash
cargo test -p robrix register::validation 2>&1 | tail -20
```

预期：全部 pass（上面 8 个 test 都应该通过）。如果 fail，检查 `url` crate 的行为是否和我假设一致（有些 trailing slash 处理和版本有关）。

- [ ] **Step 3：commit**

```bash
git add src/register/validation.rs
git commit -m "feat(register): homeserver URL normalizer + tests

Accept bare hostname (prepend https://), strip trailing slash,
reject non-http(s) schemes and empty input. 8 unit tests cover
the edge cases."
```

---

## Task 3: `RegisterAction` + `HsCapabilities` 数据类型

**Files:**
- Modify: `src/register/mod.rs`

- [ ] **Step 1：在 `src/register/mod.rs` 追加数据类型**

在 `pub fn script_mod(...)` 下方追加：

```rust
use matrix_sdk::ruma::api::client::uiaa::UiaaInfo;
use makepad_widgets::DefaultNone;

/// Homeserver capabilities discovered before register branching.
#[derive(Clone, Debug)]
pub struct HsCapabilities {
    /// Normalized base URL the client will use.
    pub base_url: String,
    /// True iff the server advertises `m.authentication.issuer` in
    /// `.well-known/matrix/client` (MSC2965 / MAS delegation).
    pub is_mas_native_oidc: bool,
    /// True iff `POST /_matrix/client/v3/register` with empty body returns
    /// 401 with parseable UIAA flows (NOT 403 M_FORBIDDEN).
    pub registration_enabled: bool,
    /// Optional UIAA probe result (empty when server requires MAS).
    pub uiaa_probe: Option<UiaaInfo>,
    /// Identity providers harvested from `/_matrix/client/v3/login`.
    /// Phase 1 populates but does not render; Phase 4 renders buttons.
    pub sso_providers: Vec<IdentityProviderSummary>,
}

/// Minimal info per identity provider. Full matrix-sdk type is not
/// used because we only need name + id at this phase.
#[derive(Clone, Debug)]
pub struct IdentityProviderSummary {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
}

/// Outcome classification of capability discovery.
///
/// Derived from `HsCapabilities` by `classify()` below; used for UI display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterMode {
    /// Server advertises MAS OAuth; register goes through browser.
    MasWebOnly,
    /// Server supports direct UIAA register.
    Uiaa,
    /// Server explicitly disallows registration.
    Disabled,
}

impl HsCapabilities {
    /// Produce a human-routable mode. Follows element-web `Registration.tsx`:
    /// MAS presence wins over UIAA even when both are possible.
    pub fn mode(&self) -> RegisterMode {
        if self.is_mas_native_oidc {
            RegisterMode::MasWebOnly
        } else if self.registration_enabled {
            RegisterMode::Uiaa
        } else {
            RegisterMode::Disabled
        }
    }
}

/// Actions produced by or consumed by the register feature.
///
/// `Cx::post_action(RegisterAction::*)` from any widget; handled by `App` and
/// by `RegisterScreen`.
#[derive(Clone, Debug, DefaultNone)]
pub enum RegisterAction {
    /// User clicked the back button on RegisterScreen.
    NavigateToLogin,
    /// Sliding-sync reports the result of capability discovery.
    CapabilitiesDiscovered(HsCapabilities),
    /// Capability discovery failed (network error, bad URL, 5xx).
    DiscoveryFailed(String),
    None,
}
```

- [ ] **Step 2：验证 `cargo build` 通过**

```bash
cargo build 2>&1 | tail -15
```

预期：无 error。`DefaultNone` 来自 `makepad_widgets` — 如果 grep `DefaultNone` 在项目中的用法并对齐（例如 `src/home/` 里常用）。

- [ ] **Step 3：commit**

```bash
git add src/register/mod.rs
git commit -m "feat(register): add RegisterAction + HsCapabilities types

Introduce the data model used across Phase 1-5:
- HsCapabilities with is_mas_native_oidc / registration_enabled /
  uiaa_probe / sso_providers fields matching the spec
- RegisterMode enum (MasWebOnly / Uiaa / Disabled) derived via
  HsCapabilities::mode() — MAS wins over UIAA per element-web rule
- RegisterAction with NavigateToLogin / CapabilitiesDiscovered /
  DiscoveryFailed variants for Phase 1 + None default"
```

---

## Task 4: `LoginAction::NavigateToRegister` 变体 + 入口按钮改派

**Files:**
- Modify: `src/login/login_screen.rs`（L1223 附近 enum，以及 "Sign up here" 按钮 click handler）

- [ ] **Step 1：在 `LoginAction` enum 追加 `NavigateToRegister` 变体**

定位：`src/login/login_screen.rs:1223`。在 `ShowAddAccountScreen` 之后、`CancelAddAccount` 之前加：

```rust
    /// User clicked "Sign up here"; the main App should hide the
    /// login screen and show RegisterScreen.
    NavigateToRegister,
```

- [ ] **Step 2：grep 定位 "Sign up here" 按钮的现有 click handler**

```bash
grep -n "set_signup_mode\|sign up here\|Sign up here" src/login/login_screen.rs | head -10
```

找到 click 分支。它当前调用 `set_signup_mode(true)`。

- [ ] **Step 3：替换 click handler**

把 `set_signup_mode(true)` 的那一整段 click 分支改成：

```rust
Cx::post_action(LoginAction::NavigateToRegister);
```

（保留任何围绕的 `if button_clicked` / `if let Some(...)` 外层结构；只替换函数体。）

> **注意**：本 Task 仅改入口。删除 signup mode 的其它残留在 Task 7 统一做，避免一次改动过大。

- [ ] **Step 4：`cargo build` 不应破坏现有代码**

```bash
cargo build 2>&1 | tail -15
```

预期：编译通过。如果出现 "未使用的函数 `set_signup_mode`" warning，Task 7 会删除。

- [ ] **Step 5：commit**

```bash
git add src/login/login_screen.rs
git commit -m "feat(login): dispatch NavigateToRegister for Sign Up click

Add LoginAction::NavigateToRegister variant. Replace
set_signup_mode(true) call with Cx::post_action dispatch so
the main App can route to the new RegisterScreen.

The signup-mode rendering code is removed in a later task."
```

---

## Task 5: `RegisterScreen` widget 完整 DSL + handle_event

**Files:**
- Modify: `src/register/register_screen.rs`（替换 Task 1 的 stub）

- [ ] **Step 1：参考 `login_screen.rs` 的 DSL 结构和命名习惯**

```bash
head -200 src/login/login_screen.rs | grep -E "script_mod|:= |widget|View" | head -20
```

留意项目常见模式：`View { width: Fill, ... }`、命名子 widget 用 `:= {{Type}}`、事件用 `click` action。

- [ ] **Step 2：替换 `src/register/register_screen.rs` 完整内容**

```rust
//! RegisterScreen widget: homeserver picker + capability display.
//!
//! Phase 1 renders:
//!   - Back button (returns to login)
//!   - Screen title
//!   - Homeserver URL input
//!   - Next button (triggers capability discovery)
//!   - Three-state status area (MAS / UIAA / Disabled / errors)
//!
//! Phases 2-5 fill in OIDC launch / UIAA form / SSO buttons.

use makepad_widgets::*;
use crate::register::{RegisterAction, RegisterMode};
use crate::register::validation::{normalize_homeserver_url, HomeserverUrlError};
use crate::sliding_sync::{submit_async_request, MatrixRequest};

script_mod! {
    use makepad_widgets::base::*;
    use makepad_widgets::theme_desktop_dark::*;

    pub RegisterScreen := {{RegisterScreen}} View {
        width: Fill,
        height: Fill,
        flow: Flow.Down,
        padding: Inset { top: 24., right: 32., bottom: 24., left: 32. },
        spacing: 16.,
        show_bg: true,
        draw_bg: { color: #x1F2124 }

        back_button := <Button> {
            width: Fit, height: Fit,
            text: "← Back to Login",
            draw_text: { color: #xAAB2BE }
        }

        title := <Label> {
            width: Fit, height: Fit,
            draw_text: {
                color: #xF1F2F3,
                text_style: { font_size: 22. }
            },
            text: "Create Account"
        }

        homeserver_row := <View> {
            width: Fill, height: Fit,
            flow: Flow.Down, spacing: 4.,

            <Label> {
                text: "Homeserver URL",
                draw_text: { color: #xAAB2BE, text_style: { font_size: 12. } }
            }

            homeserver_input := <TextInput> {
                width: Fill, height: 40.,
                empty_message: "matrix.org",
            }
        }

        next_button := <Button> {
            width: Fit, height: Fit,
            text: "Next",
        }

        status_area := <View> {
            width: Fill, height: Fit,
            flow: Flow.Down, spacing: 8.,
            visible: false,

            status_label := <Label> {
                width: Fill, height: Fit,
                draw_text: {
                    color: #xE0E3E8,
                    text_style: { font_size: 14. }
                },
                text: ""
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RegisterScreen {
    #[deref] view: View,

    /// Cached capabilities from last successful discovery; None before first probe.
    #[rust] last_discovery: Option<crate::register::HsCapabilities>,
}

impl Widget for RegisterScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.handle_actions(cx, event);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl RegisterScreen {
    fn handle_actions(&mut self, cx: &mut Cx, event: &Event) {
        let actions = event.actions();
        let back = self.view.button(ids!(back_button));
        let next = self.view.button(ids!(next_button));

        if back.clicked(&actions) {
            Cx::post_action(RegisterAction::NavigateToLogin);
            return;
        }

        if next.clicked(&actions) {
            let raw = self.view.text_input(ids!(homeserver_input)).text();
            match normalize_homeserver_url(&raw) {
                Ok(url) => {
                    self.show_status(cx, "Checking server capabilities...");
                    submit_async_request(MatrixRequest::DiscoverHomeserverCapabilities {
                        url: url.as_str().to_string(),
                    });
                }
                Err(HomeserverUrlError::Empty) => {
                    self.show_status(cx, "Please enter a homeserver URL (e.g. matrix.org).");
                }
                Err(HomeserverUrlError::UnsupportedScheme(s)) => {
                    self.show_status(cx, &format!("Unsupported scheme: {s}. Only http(s) is allowed."));
                }
                Err(HomeserverUrlError::Invalid) => {
                    self.show_status(cx, "That URL looks invalid. Please check and try again.");
                }
            }
        }

        // Capability discovery results from sliding_sync.
        for action in actions {
            match action.cast() {
                RegisterAction::CapabilitiesDiscovered(caps) => {
                    let msg = match caps.mode() {
                        RegisterMode::MasWebOnly => {
                            "This server uses browser-based registration (MAS OAuth). Phase 2 will handle this."
                        }
                        RegisterMode::Uiaa => {
                            "This server allows direct account creation. Phase 3 will handle the form."
                        }
                        RegisterMode::Disabled => {
                            "This server does not allow registration. Please choose a different homeserver or sign in with an existing account."
                        }
                    };
                    self.show_status(cx, msg);
                    self.last_discovery = Some(caps);
                }
                RegisterAction::DiscoveryFailed(err) => {
                    self.show_status(cx, &format!("Could not reach that server: {err}"));
                    self.last_discovery = None;
                }
                _ => {}
            }
        }
    }

    fn show_status(&mut self, cx: &mut Cx, message: &str) {
        self.view.view(ids!(status_area)).set_visible(cx, true);
        self.view.label(ids!(status_label)).set_text(cx, message);
        self.view.redraw(cx);
    }
}
```

> **执行者注意**：`submit_async_request`、`MatrixRequest::DiscoverHomeserverCapabilities` 此时尚未定义（在 Task 6 加）。本 Task 代码会 `cargo build` **失败**——这是预期的。下一步验证编译失败点和预期消息对得上，然后继续 Task 6。

- [ ] **Step 3：验证 cargo build 的错误是"预期的"**

```bash
cargo build 2>&1 | grep -E "error\[|no variant|no function" | head -10
```

预期 error：`no variant named \`DiscoverHomeserverCapabilities\` found for enum \`MatrixRequest\``。 这条 error 确认 RegisterScreen 代码已经接入了请求调度系统；Task 6 补齐 MatrixRequest 即可消除。

> **不 commit，本 Task 的 commit 和 Task 6 合并。**

---

## Task 6: `MatrixRequest::DiscoverHomeserverCapabilities` + handler

**Files:**
- Modify: `src/sliding_sync.rs`（L695 enum + worker loop match arm）

- [ ] **Step 1：`MatrixRequest` enum 追加变体**

定位 `src/sliding_sync.rs:695`。在 `MatrixRequest::Login(LoginRequest)` 之前（或之后，保持枚举可读）加：

```rust
    /// Request to probe a homeserver's registration capabilities.
    /// Sent from RegisterScreen's Next button; result arrives as
    /// `RegisterAction::CapabilitiesDiscovered` or `DiscoveryFailed`.
    DiscoverHomeserverCapabilities {
        /// Already-normalized homeserver URL (has scheme).
        url: String,
    },
```

- [ ] **Step 2：在 worker loop 的 match 里加新分支**

定位 `src/sliding_sync.rs:1288`（`match request { ... MatrixRequest::Login(login_request) => { ... } ...`）。在该 match 的**末尾**（`}` 前）加：

```rust
            MatrixRequest::DiscoverHomeserverCapabilities { url } => {
                tokio::spawn(async move {
                    use crate::register::{RegisterAction, HsCapabilities, IdentityProviderSummary};

                    match discover_homeserver_capabilities(&url).await {
                        Ok(caps) => {
                            Cx::post_action(RegisterAction::CapabilitiesDiscovered(caps));
                        }
                        Err(e) => {
                            Cx::post_action(RegisterAction::DiscoveryFailed(e.to_string()));
                        }
                    }
                });
            }
```

- [ ] **Step 3：在 `sliding_sync.rs` 文件末尾（在 `matrix_worker_task` 函数外）加 helper 函数**

```rust
/// Probe a homeserver's registration capabilities.
///
/// Fetches in order:
/// 1. GET `.well-known/matrix/client` — discover base_url and MAS issuer
/// 2. GET `/_matrix/client/versions` — liveness check
/// 3. GET `/_matrix/client/v3/login` — enumerate top-level flows (SSO providers)
/// 4. POST `/_matrix/client/v3/register` with empty body — harvest UIAA flows
///
/// Each step is lenient: a failure in steps 1-3 falls back to reasonable
/// defaults; only step 4 error or total network failure bubbles up.
async fn discover_homeserver_capabilities(
    raw_url: &str,
) -> anyhow::Result<crate::register::HsCapabilities> {
    use crate::register::{HsCapabilities, IdentityProviderSummary};
    use serde_json::Value;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Step 1: .well-known (lenient — default base_url = raw_url on failure).
    let wk_url = format!("{raw_url}/.well-known/matrix/client");
    let (base_url, is_mas) = match http.get(&wk_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            let base = body
                .get("m.homeserver")
                .and_then(|m| m.get("base_url"))
                .and_then(|v| v.as_str())
                .unwrap_or(raw_url)
                .trim_end_matches('/')
                .to_string();
            let mas = body
                .get("m.authentication")
                .and_then(|m| m.get("issuer"))
                .and_then(|v| v.as_str())
                .is_some();
            (base, mas)
        }
        _ => (raw_url.trim_end_matches('/').to_string(), false),
    };

    // Step 2: versions (liveness; we only care about 2xx).
    let versions_url = format!("{base_url}/_matrix/client/versions");
    http.get(&versions_url)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("homeserver unreachable: {e}"))?;

    // Step 3: /v3/login — SSO providers (non-fatal on failure).
    let login_url = format!("{base_url}/_matrix/client/v3/login");
    let sso_providers = match http.get(&login_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            body.get("flows")
                .and_then(|f| f.as_array())
                .map(|flows| {
                    flows
                        .iter()
                        .filter(|f| {
                            f.get("type").and_then(|t| t.as_str()) == Some("m.login.sso")
                        })
                        .flat_map(|f| {
                            f.get("identity_providers")
                                .and_then(|ip| ip.as_array())
                                .cloned()
                                .unwrap_or_default()
                        })
                        .filter_map(|p| {
                            Some(IdentityProviderSummary {
                                id: p.get("id")?.as_str()?.to_string(),
                                name: p
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                icon_url: p
                                    .get("icon")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };

    // Step 4: POST /register empty body — harvest UIAA flows.
    let register_url = format!("{base_url}/_matrix/client/v3/register");
    let reg_resp = http
        .post(&register_url)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await?;

    let status = reg_resp.status();
    let body: Value = reg_resp.json().await.unwrap_or(Value::Null);

    let (registration_enabled, uiaa_probe) = if status == reqwest::StatusCode::UNAUTHORIZED {
        // Expected UIAA challenge. Parse into ruma UiaaInfo.
        match serde_json::from_value(body.clone()) {
            Ok(info) => (true, Some(info)),
            Err(_) => (true, None), // server returned 401 but odd shape; treat as enabled
        }
    } else if status == reqwest::StatusCode::FORBIDDEN
        && body.get("errcode").and_then(|v| v.as_str()) == Some("M_FORBIDDEN")
    {
        (false, None)
    } else {
        // Unexpected status — treat cautiously as disabled.
        (false, None)
    };

    Ok(HsCapabilities {
        base_url,
        is_mas_native_oidc: is_mas,
        registration_enabled,
        uiaa_probe,
        sso_providers,
    })
}
```

> **注意**：`reqwest` 已作为 matrix-sdk 的 transitive dep 存在，但可能没有直接在 `Cargo.toml` 声明。如果 `cargo build` 报 `unresolved import reqwest`，加一行到 `Cargo.toml` `[dependencies]`：`reqwest = { version = "0.12", features = ["json"] }`（检查 matrix-sdk 实际拉取的 reqwest 版本，对齐它）。但**若 CLAUDE.md / project.spec.md 禁止新增 Cargo 依赖**，则改用 matrix-sdk 的 `Client::http_client()` 替代；本 plan 草案假设 reqwest 已可直接用。

- [ ] **Step 4：`cargo build` 通过**

```bash
cargo build 2>&1 | tail -15
```

预期：无 error。Task 5 + Task 6 合并编译通过。若 reqwest 需要显式声明，按上条注释处理。

- [ ] **Step 5：commit（Task 5 + Task 6 一起）**

```bash
git add src/register/register_screen.rs src/sliding_sync.rs
git commit -m "feat(register): capability discovery + RegisterScreen UI

Implement MatrixRequest::DiscoverHomeserverCapabilities handler
that probes .well-known, /versions, /v3/login, and empty POST
/register to build HsCapabilities. Results post back as
RegisterAction::CapabilitiesDiscovered / DiscoveryFailed.

RegisterScreen widget renders the homeserver input, Next button,
and three-state status area (MAS / UIAA / Disabled / errors).
The full wizard body is added in Phase 2+."
```

---

## Task 7: 删除 `login_screen.rs` 的 signup mode 残留

**Files:**
- Modify: `src/login/login_screen.rs`（删除约 80 行）

- [ ] **Step 1：定位 signup mode 相关代码段**

```bash
grep -n "set_signup_mode\|confirm_password\|signup_mode" src/login/login_screen.rs | head -20
```

常见位置：
- DSL 里的 `confirm_password_input := <TextInput>` 字段
- DSL 里的 mode toggle 按钮（形如 `mode_toggle_button`）
- Rust 侧的 `fn set_signup_mode(&mut self, cx: &mut Cx, is_signup: bool)`
- `handle_event` 里的 `if button_clicked(mode_toggle_button) { ... }` 分支
- `handle_event` 里依 signup 模式分支的 submit 逻辑（password vs RegisterAccount）

- [ ] **Step 2：逐段删除**

删掉以下各处（具体行号以 grep 结果为准）：

1. DSL 里 `confirm_password_input` 的定义整块
2. DSL 里 `mode_toggle_button` 的定义整块
3. DSL 里任何 `signup_mode_only := <View>` 类包装
4. Rust 侧 `fn set_signup_mode` 整个函数体
5. Rust 侧 `handle_event` / `handle_actions` 中触发 signup 提交的 match 分支（形如 `LoginRequest::Register(...)`）——注意保留 **登录** 路径（LoginByPassword）
6. 结构体字段 `is_signup_mode: bool`（如果有）及其 `#[rust]` 标注
7. `login_screen.rs` 顶部 import 里用不到的东西（`RegisterAccount` 等）

> **不删** "Sign up here" 按钮本身（Task 4 保留了它；它的 click 已改为 NavigateToRegister）。

- [ ] **Step 3：验证登录路径仍然工作**

```bash
cargo build 2>&1 | tail -15
```

预期：无 error。warning 可能提到 `unused import RegisterAccount` 等，把它们也删掉。

- [ ] **Step 4：手工测试 — 运行 robrix2 验证普通密码登录不破**

```bash
cargo run 2>&1 | tail -30
```

用已有 Matrix 账号登录，验证：
- 登录屏正常显示
- 输入 user_id 和 password 能成功登录
- SSO 按钮仍能工作
- 顶端的 "Sign up here" 按钮仍然在原位置，文字和样式未变

- [ ] **Step 5：在本次测试用户确认登录无回归后 commit**

等用户 OK 后：

```bash
git add src/login/login_screen.rs
git commit -m "refactor(login): remove signup mode

Delete confirm_password input, mode toggle button, set_signup_mode
function, and signup-submit branch. The \"Sign up here\" button
retains its text/position/style; its click now posts
LoginAction::NavigateToRegister instead.

Registration logic moves wholesale to src/register/ — see
specs/task-register-flow.spec.md."
```

---

## Task 8: `App` 导航 — 登录屏 ↔ 注册屏

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1：在 app DSL 里加 `RegisterScreen` 子 widget**

```bash
grep -n "LoginScreen\|login_screen" src/app.rs | head -10
```

找到 `LoginScreen` widget 在 app DSL 中声明的位置。按同级结构加上：

```makepad
register_screen = <RegisterScreen> {
    visible: false,
}
```

（放在 `login_screen` 定义之后；同一父 View 内；`visible: false` 作为默认隐藏。）

- [ ] **Step 2：导入 `RegisterScreen` 的 widget 注册**

确认 app.rs 文件顶部有 `use crate::register;` 或类似，使 `script_mod` 能找到 RegisterScreen。如果没有，加一行。

- [ ] **Step 3：在 App 的 `handle_actions` 处理 `LoginAction::NavigateToRegister` 和 `RegisterAction::NavigateToLogin`**

```bash
grep -n "handle_actions\|handle_event" src/app.rs | head -10
```

定位到 `MatchEvent` impl 或 `handle_actions` 函数，加：

```rust
use crate::register::RegisterAction;
use crate::login::login_screen::LoginAction;

// Inside handle_actions:
for action in actions {
    match action.cast() {
        LoginAction::NavigateToRegister => {
            self.ui.view(ids!(login_screen)).set_visible(cx, false);
            self.ui.view(ids!(register_screen)).set_visible(cx, true);
            self.ui.redraw(cx);
        }
        _ => {}
    }
    match action.cast() {
        RegisterAction::NavigateToLogin => {
            self.ui.view(ids!(register_screen)).set_visible(cx, false);
            self.ui.view(ids!(login_screen)).set_visible(cx, true);
            self.ui.redraw(cx);
        }
        _ => {}
    }
}
```

> 执行者注意：具体 `ids!(register_screen)` 的嵌套路径取决于 app DSL 结构。如果 app 有中间 View（如 `root_view`），路径要写全。

- [ ] **Step 4：`cargo build` 通过**

```bash
cargo build 2>&1 | tail -15
```

- [ ] **Step 5：手工测试 — 验证导航**

```bash
cargo run 2>&1 | tail -30
```

验证：
- 启动 → 登录屏
- 点 "Sign up here" → 登录屏隐藏，注册屏显示（只有空白框架 + 输入框 + Next 按钮）
- 点注册屏上的 "← Back to Login" → 注册屏隐藏，登录屏显示

- [ ] **Step 6：用户确认导航功能 OK 后 commit**

```bash
git add src/app.rs
git commit -m "feat(app): wire login <-> register navigation

Handle LoginAction::NavigateToRegister and
RegisterAction::NavigateToLogin to toggle visibility between
LoginScreen and RegisterScreen. Both screens live under the same
parent view; default visibility stays on LoginScreen."
```

---

## Task 9: 端到端手工验证（Phase 1 DoD）

**Files:** 无改动；仅验证 + 最终 commit（若有遗漏）。

- [ ] **Step 1：启动 robrix2**

```bash
cargo run 2>&1 | tail -30
```

- [ ] **Step 2：验证场景 A — MAS OAuth 服务器（用你的 alvin.meldry.com）**

- 点 "Sign up here"
- 在 Homeserver URL 输入 `alvin.meldry.com` （bare hostname）
- 点 "Next"
- **预期**：status area 显示 "This server uses browser-based registration (MAS OAuth). Phase 2 will handle this."

- [ ] **Step 3：验证场景 B — matrix.org（也是 MAS）**

- 点 "← Back to Login"
- 再点 "Sign up here"
- 输入 `matrix.org`
- 点 "Next"
- **预期**：同 alvin.meldry.com，显示 MAS 消息

- [ ] **Step 4：验证场景 C — 不存在的服务器**

- 输入 `this-server-does-not-exist.example`
- 点 "Next"
- **预期**：status area 显示 "Could not reach that server: ..." 错误

- [ ] **Step 5：验证场景 D — 非法 URL**

- 输入 `ftp://example.com`
- 点 "Next"
- **预期**：status area 显示 "Unsupported scheme: ftp. Only http(s) is allowed."

- [ ] **Step 6：验证场景 E — 空输入**

- 清空输入框
- 点 "Next"
- **预期**：status area 显示 "Please enter a homeserver URL (e.g. matrix.org)."

- [ ] **Step 7：回归验证 — 密码登录仍工作**

- 点 "← Back to Login"
- 用已有账号（user_id + password）登录
- **预期**：登录成功进入主屏

- [ ] **Step 8：用户 sign off**

所有 6 个场景 + 回归场景都通过后，和用户确认"阶段 1 验收通过"，然后 commit any trailing cleanup：

```bash
# 如果之前 task 的测试 / lint 遗漏任何 warning：
cargo build 2>&1 | grep warning | head -10
# 修掉后：
git status
git add <...>
git commit -m "chore(register): post-review cleanup for Phase 1"
# 如果无需 cleanup，跳过该 commit。
```

- [ ] **Step 9：运行 agent-spec verify 对齐 spec 的阶段 1 场景**

```bash
agent-spec explain specs/task-register-flow.spec.md --scenario "[Phase 1]" 2>&1 | head -20
```

对照 spec 的 3 个 Phase 1 场景（`manual_test_phase1_hs_discovery`、`manual_test_phase1_registration_disabled`、`manual_test_signup_entry_button_layout`）逐条核对。

---

## Self-Review

### Spec coverage（对照 `specs/task-register-flow.spec.md`）

| Spec 要素 | 对应 Task |
|---|---|
| 创建 `src/register/mod.rs` | Task 1 + Task 3 |
| 创建 `src/register/register_screen.rs` | Task 1 + Task 5 |
| 创建 `src/register/register_status_modal.rs` | Task 1（scaffold 已创建，Phase 2 再填 body） |
| 创建 `src/register/validation.rs` | Task 2 |
| `src/register/oidc.rs` / `src/register/uiaa.rs` | ❌ 阶段 2/3，不在本 plan |
| `src/lib.rs` 加 `pub mod register;` | Task 1 |
| `src/app.rs` 处理 NavigateToRegister / NavigateToLogin | Task 8 |
| `src/login/login_screen.rs` 删 signup mode + 改 click | Task 4（click） + Task 7（删除残留） |
| `MatrixRequest::DiscoverHomeserverCapabilities` + handler | Task 6 |
| Scenario `manual_test_signup_entry_button_layout` | Task 4 + Task 9 Step 7 |
| Scenario `manual_test_phase1_hs_discovery` | Task 6 + Task 9 Step 2-3 |
| Scenario `manual_test_phase1_registration_disabled` | Task 6 + Task 9 Step 4 |

### Placeholder scan

无 TBD / TODO-without-code / "类似 Task N" — 每条 step 都带可执行命令或可粘代码。reqwest dep 的注释是条件分支（如果需要就加，不需要就跳），不是 placeholder。

### Type consistency

- `RegisterAction::CapabilitiesDiscovered(HsCapabilities)`：Task 3 定义，Task 5 / Task 6 使用 — 签名一致
- `HsCapabilities::mode() -> RegisterMode`：Task 3 定义，Task 5 `handle_actions` 使用 — 一致
- `MatrixRequest::DiscoverHomeserverCapabilities { url: String }`：Task 6 定义，Task 5 使用 — 字段名一致
- `LoginAction::NavigateToRegister`：Task 4 定义，Task 8 app.rs 使用 — 一致
- `RegisterAction::NavigateToLogin`：Task 3 定义，Task 5 发出，Task 8 接收 — 一致

### 依赖 review

- `zxcvbn`：**阶段 1 不需要**，延迟到阶段 5
- `reqwest`：本 plan 默认作为 matrix-sdk transitive dep 直接用；若 cargo 抱怨则显式加进 `Cargo.toml`。这个决定需要执行者验证——本 plan 第一次接触 Cargo.toml 新增依赖的风险点
- 无其它新依赖

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-21-register-phase-1-skeleton.md`.

**Two execution options:**

**1. Subagent-Driven (recommended)** — 每 Task 派一个 fresh subagent 执行，Task 之间可以 review；适合 9 个原子 Task 按顺序推进

**2. Inline Execution** — 本 session 继续直接执行，每几个 Task 一个 checkpoint 让用户 review；批量更快但 session 可能膨胀

选哪个？
