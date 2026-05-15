# Register Phase 2: MAS Browser-Based Registration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 当 homeserver 是 MAS 类(stable `m.authentication` 或 unstable `org.matrix.msc2965.authentication`),点 Next 之后自动打开系统浏览器到服务器的账户管理页(`account` URL 或 `<issuer>/account/`),用户在网页上完成注册,回 robrix2 用新账号手动登录。

**Architecture:**
1. `HsCapabilities` 扩展一个 `mas_account_url: Option<String>` 字段,在 Phase 1 已有的 `.well-known` 解析同一处填充,不增加额外 HTTP 往返。
2. `RegisterScreen` 的 `CapabilitiesDiscovered(caps)` handler 在 `MasWebOnly` 分支里调用 `robius_open::Uri::new(&url).open()`,状态区切换到"浏览器已打开"指引。
3. 不做 OAuth callback、不做 token exchange、不引入新依赖。Element Desktop 完整 callback 版的实现放在未来的 Phase 2.5(独立 PR)。

**Tech Stack:** Phase 1 全部依赖 + `robius_open`(已在 `src/sliding_sync.rs:2791` / `src/home/room_screen.rs:6758` / `src/settings/settings_screen.rs:774` 多处使用,模式统一:`robius_open::Uri::new(&url).open()` → `Result<(), _>`)。

---

## File Structure

| 文件 | 责任 |
|---|---|
| `src/register/mod.rs` | 扩 `HsCapabilities` 字段 |
| `src/sliding_sync.rs` | `discover_homeserver_capabilities` 内 populate `mas_account_url` |
| `src/register/register_screen.rs` | `MasWebOnly` 分支从"显示占位文本"改为"启动浏览器 + 新状态文本" |

无新文件、无新 action、无新 MatrixRequest。

---

## Task 1: HsCapabilities 加 mas_account_url 字段

**Files:**
- Modify: `src/register/mod.rs`

- [ ] **Step 1:struct 加字段**

在 `HsCapabilities` 的字段列表末尾加:

```rust
/// URL to open in the system browser for MAS registration.
/// Populated from `.well-known` `m.authentication.account` (or the
/// unstable variant), with fallback to `<issuer>/account/`. None when
/// the server is not MAS (non-MasWebOnly modes).
pub mas_account_url: Option<String>,
```

放在 `pub sso_providers: Vec<IdentityProviderSummary>,` 之后。

- [ ] **Step 2:确认构造处全部显式填 None**

```bash
grep -rn "HsCapabilities {" src/
```

如果有别的地方构造 HsCapabilities struct literal(比如测试),把 `mas_account_url: None` 加上。

- [ ] **Step 3:build 通过**

```bash
cargo build 2>&1 | tail -10
```

- [ ] **Step 4:commit**

```bash
git add src/register/mod.rs
git commit -m "feat(register): HsCapabilities carries mas_account_url"
```

---

## Task 2: Discovery 填充 mas_account_url

**Files:**
- Modify: `src/sliding_sync.rs`

- [ ] **Step 1:定位现有 MAS 检测代码**

```bash
grep -n "m.authentication\|msc2965" src/sliding_sync.rs
```

应该在 `discover_homeserver_capabilities` 里找到之前修过的 `["m.authentication", "org.matrix.msc2965.authentication"].iter().any(...)` 循环(位置大约 L6250-L6264 附近,具体行号 grep 为准)。

- [ ] **Step 2:把检测 + URL 提取融合成一次遍历**

当前代码只是 `.is_some()` 布尔判断。改成"找到第一个匹配的 key,同时拿它的 issuer + account":

```rust
// Replace the existing .any(|key| ...) block with:
let (is_mas, mas_account_url) = ["m.authentication", "org.matrix.msc2965.authentication"]
    .iter()
    .find_map(|key: &&str| {
        let block = body.get(*key)?;
        let issuer = block.get("issuer").and_then(|v: &Value| v.as_str())?;
        // Prefer the explicit `account` field; fall back to <issuer>/account/.
        let account = block
            .get("account")
            .and_then(|v: &Value| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{}/account/", issuer.trim_end_matches('/')));
        Some((true, Some(account)))
    })
    .unwrap_or((false, None));
let mas = is_mas;
```

要点:
- 用 `find_map` 取第一个匹配到 `issuer` 的 block(稳定 key 优先)。
- `account` 字段不存在时 fallback `<issuer>/account/`(`trim_end_matches('/')` 防止双斜杠)。
- 保持变量名 `mas` 与后续代码兼容(`HsCapabilities.is_mas_native_oidc` 仍然用它)。

- [ ] **Step 3:HsCapabilities 构造处把 mas_account_url 写进去**

找到 `Ok(HsCapabilities { ... })` 那块(函数末尾),加一行:

```rust
Ok(HsCapabilities {
    base_url,
    is_mas_native_oidc: is_mas,
    registration_enabled,
    uiaa_probe,
    sso_providers,
    mas_account_url,     // <- new
})
```

- [ ] **Step 4:build 通过**

```bash
cargo build 2>&1 | tail -10
```

零新 warning。

- [ ] **Step 5:commit**

```bash
git add src/sliding_sync.rs
git commit -m "feat(register): extract mas_account_url from .well-known"
```

---

## Task 3: RegisterScreen MasWebOnly 分支启动浏览器

**Files:**
- Modify: `src/register/register_screen.rs`

- [ ] **Step 1:定位现有 MasWebOnly 分支**

```bash
grep -n "MasWebOnly\|CapabilitiesDiscovered" src/register/register_screen.rs
```

当前代码形如:

```rust
RegisterMode::MasWebOnly => "This server uses browser-based registration (MAS OAuth). Phase 2 will handle this.",
```

是 `match caps.mode() { ... }` 表达式内的一个 arm,返回一个 `&str` 给后面的 `self.show_status(cx, msg)`。

- [ ] **Step 2:把 match 表达式重构为 if-else 式副作用**

因为 MasWebOnly 分支需要从 `caps` 读 URL + 做副作用(打开浏览器),单纯返回 `&str` 不够。改成:

```rust
match caps.mode() {
    RegisterMode::MasWebOnly => {
        match caps.mas_account_url.as_deref() {
            Some(url) => {
                match robius_open::Uri::new(url).open() {
                    Ok(()) => {
                        self.show_status(
                            cx,
                            "Browser opened. Complete registration in your web browser, \
                             then click ← Back to Login and sign in with your new account.",
                        );
                    }
                    Err(e) => {
                        log!("robius_open failed for MAS signup url {url}: {e}");
                        self.show_status(
                            cx,
                            &format!(
                                "Could not open the browser. Please visit this URL manually:\n{url}"
                            ),
                        );
                    }
                }
            }
            None => {
                // 不应到达:is_mas_native_oidc = true 时 discovery 必定填了 url
                self.show_status(
                    cx,
                    "This server advertises browser-based registration but no signup URL was found.",
                );
            }
        }
    }
    RegisterMode::Uiaa => {
        self.show_status(
            cx,
            "This server allows direct account creation. Phase 3 will handle the form.",
        );
    }
    RegisterMode::Disabled => {
        self.show_status(
            cx,
            "This server does not allow registration. Please choose a different homeserver \
             or sign in with an existing account.",
        );
    }
}
self.last_discovery = Some(caps.clone());
```

要点:
- `log!` 宏来自 `makepad_widgets`,已在其它文件用过;若当前文件没 import,加 `use makepad_widgets::log;` 或用 `makepad_widgets::log!` 完整路径。
- 错误文案里直接把 URL 给用户(方便复制粘贴),不要截断。
- `self.last_discovery = Some(caps.clone())` 保留原行为(Phase 4 会用它来渲染 SSO 按钮,此时无影响)。

- [ ] **Step 3:build 通过**

```bash
cargo build 2>&1 | tail -10
```

零新 warning。如果 `log!` 或 `robius_open` import 缺失,顶上 import 区加:

```rust
use makepad_widgets::*;  // 已有,log! 通常透过这个
// robius_open 已在 matrix-sdk 的 transitive deps 里,不需要新 Cargo.toml
```

- [ ] **Step 4:cargo run 跑一下(确认 DSL 不炸)**

```bash
cargo run 2>&1 | head -80
```

看 register_screen.rs 相关行无 Makepad VM 报错。

- [ ] **Step 5:不要 commit,等 Task 4 用户端到端验证通过再一起 commit**

---

## Task 4: 端到端手工验证

**Files:** 无改动;跑 app。

- [ ] **Step 1:启动 robrix2**

```bash
cargo run 2>&1 | tail -30
```

- [ ] **Step 2:场景 A — alvin.meldry.com 完整注册流**

1. 点 "Sign up here"
2. 输入 `alvin.meldry.com`,点 "Next"
3. **预期 UI**:状态区显示 *"Browser opened. Complete registration in your web browser, then click ← Back to Login and sign in with your new account."*
4. **预期浏览器**:系统浏览器自动打开到 `https://auth.alvin.meldry.com/account/`(或 `/register` 从该页进入)
5. 在浏览器完成注册(随便 user_id + password)
6. 回 robrix2,点 "← Back to Login"
7. 用刚注册的账号登录 → 进入主屏

- [ ] **Step 3:场景 B — matrix.org 到达注册页即可**

1. 返回注册屏,输 `matrix.org`,"Next"
2. **预期**:浏览器打开 `https://account.matrix.org/account/`,状态区显示"Browser opened..."
3. 注册页可见即可(不需要真注册)

- [ ] **Step 4:场景 C — 回归 Phase 1 的错误路径**

确认 Phase 1 三条错误路径仍然生效:
- 空输入:*"Please enter a homeserver URL (e.g. matrix.org)."*
- 非法 scheme `ftp://example.com`:*"Unsupported scheme: ftp. Only http(s) is allowed."*
- 不存在服务器 `this-server-does-not-exist.example`:*"Could not reach that server: ..."*

- [ ] **Step 5:场景 D — 登录回归**

"← Back to Login" → 用已有账号密码登录 → 进主屏。(Phase 1 已验证过,再确认一次没退化。)

- [ ] **Step 6:用户 sign off + commit Task 3 的改动**

所有场景 OK 后:

```bash
git add src/register/register_screen.rs
git commit -m "feat(register): open system browser for MAS signup

When CapabilitiesDiscovered reports MasWebOnly, launch the MAS
account URL via robius_open so the user can complete registration
in their browser. After signup the user returns to robrix2 and
logs in with the new account via the existing password flow.
This is the simple-version scope: no OAuth callback, no token
exchange — those are future Phase 2.5 if needed."
```

- [ ] **Step 7:最终 PR**

Phase 1 + Phase 2 全部落在 `fix/register` 分支,push 并开 PR 打到 main:

```bash
git push -u origin fix/register
gh pr create --title "feat(register): align with element-desktop — Phase 1+2" ...
```

PR 描述对照 Phase 1 + Phase 2 各自的成果。

---

## Self-Review

### 覆盖 spec(`specs/task-register-flow.spec.md`)的 Phase 2 范围?

- ✅ MAS 类服务器发现后自动进入浏览器路径
- ✅ 不引入 webview 依赖(符合 spec 禁止条款)
- ✅ 复用 `robius_open`(符合 spec 的 "不增加依赖"条款)
- ❌ Spec 里 OIDC 一节提到的 `loopback HTTP` / `code 换 token`——这些是 Phase 2.5 的完整 callback 范围,本 plan 显式 scope-out
- ❌ Spec 里 `oidc.rs` 单独文件——本 plan 不创建,流程足够简单不必新增文件

### 类型一致性
- `HsCapabilities.mas_account_url: Option<String>` 在 mod.rs 声明、sliding_sync.rs 填充、register_screen.rs 消费,三处类型一致 ✓
- `robius_open::Uri::new(&str).open() -> Result<(), _>` 使用方式与 `src/sliding_sync.rs:2791` 对齐 ✓

### Placeholder 检查
无 TBD / TODO / 占位符 ✓

### 遗漏?
- 没处理用户在浏览器注册期间切换 homeserver 又点 Next 的 race——这是 acceptable 的(每次 Next 都是独立 discovery → 独立 browser open),不需要特殊 state。
- 没处理用户反复点 Next(会反复开浏览器标签页)——可接受,不值得加 rate-limit。
