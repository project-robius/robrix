# Robrix: a Rust Matrix client built atop [Robius](https://github.com/project-robius)

Robrix is a Matrix chat client written in Rust to demonstrate the functionality of the Robius, a framework for multi-platform application development in Rust.

> ⚠️ Robrix is just getting started and is not yet fully functional.
>
> It is currently based on the [Makepad WeChat example](https://github.com/project-robius/makepad_wechat); only the first "Rooms" tab is in use.

## Building and Running

First, [install Rust](https://www.rust-lang.org/tools/install).

Then, on desktop platforms (macOS, Linux, Windows), simply run:
```sh
cargo run -- 'USERNAME' 'PASSWORD' ['HOMESERVER_URL']
```

* Robrix only supports a standard username + password login currently.
    * Note the usage of **single quotes** (not double quotes), which will prevent your shell from treating certain symbols as globs/regex patterns.
    * If you created your Matrix account using a third-party Single Sign On (SSO) like a Google account, you can set a standard password by using [Element's password reset form](https://app.element.io/#/forgot_password).
* The `HOMESERVER_URL` argument is optional and uses the `"https://matrix-client.matrix.org/"` URL by default.


### Building Robrix for Android

1. Install the `cargo-makepad` build tool:
   ```sh
   cargo install --force --git https://github.com/makepad/makepad.git --branch rik cargo-makepad
   ```

2. Use `cargo-makepad` to install the Android toolchain, with the full NDK:
   ```sh
   cargo makepad android install-toolchain --full-ndk
   ```

3. Build and run Robrix using `cargo-makepad`:
    * Fill in your username and password in the [`login.toml`](login.toml) file.
    * Then use cargo makepad to build and run:
       ```sh
       cargo makepad android run -p robrix --release
       ```
    * You'll need to connect a physical Android device with developer options enabled, or start up an emulator using Android Studio.
        * API version 33 or higher is required, which is Android 13 and up.


## Feature status tracker 

These are generally sorted in order of priority. If you're interested in helping out with anything here, please reach out via a GitHub issue or on our Robius matrix channel.

### Basic room views and fundamental actions
- [x] View list of joined rooms
- [x] View timeline of events in a single room
- [x] Fetch and display room avatars
- [x] Fetch user profiles (displayable names)
- [x] Fetch and display user profile avatars
- [x] Backwards pagination (upon viewing a room timeline)
- [ ] Dynamic backwards pagination based on scroll position/movement
- [ ] Loading animation while waiting for pagination request
- [x] Stable positioning of events view during timeline update
- [x] Display simple text-only messages
- [x] Display image messages (PNG, JPEG)
- [ ] Rich text formatting for message bodies
- [ ] Display multimedia (audio/video/gif) message events
- [ ] Display reactions (annotations)
- [ ] Inline link previews
- [ ] Inline reply view
- [x] Send messages (standalone, no replies)
- [ ] Interactive reaction button, send reactions
- [ ] Reply button, send reply
- [ ] Error display banners: no connection, failure to login, sync timeout.
- [ ] Collapsible/expandable view of contiguous "small" events
- [ ] Encrypted rooms, decrypting messages

### Auxiliary/admin features: login, registration, settings
- [ ] Username/password login screen
- [ ] SSO, other 3rd-party auth providers login screen
- [ ] Dedicated view of spaces
- [ ] Dedicated view of direct messages (DMs)
- [ ] Search messages
- [ ] Room browser / search
- [ ] Room creation
- [ ] Room settings/info screen
- [ ] Room members pane
- [ ] User profile settings screen
- [ ] Persistence of app state to disk

