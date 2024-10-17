# Robrix: a Rust Matrix client built atop [Robius](https://github.com/project-robius)

[![Robrix Matrix Chat](https://img.shields.io/matrix/robius-robrix%3Amatrix.org?server_fqdn=matrix.org&style=flat&logo=matrix&label=Robrix%20Matrix%20Chat&color=B7410E)](https://matrix.to/#/#robius-robrix:matrix.org)
[![Project Robius Matrix Chat](https://img.shields.io/matrix/robius-general%3Amatrix.org?server_fqdn=matrix.org&style=flat&logo=matrix&label=Project%20Robius%20Matrix%20Chat&color=B7410E)](https://matrix.to/#/#robius:matrix.org)

Robrix is a Matrix chat client written in Rust to demonstrate the functionality of [Project Robius](https://github.com/project-robius), a framework for multi-platform application development in Rust. Robrix is written using the [Makepad UI toolkit](https://github.com/makepad/makepad/).

▶️  [Click here to see the Robrix project tracker!](https://github.com/orgs/project-robius/projects/4/)

> [!NOTE]
> ⚠️ Robrix is a work-in-progress that doesn't yet support all Matrix chat features.

Check out our most recent talks and presentations for more info:
  * Robrix: a pure Rust multi-platform app for chat and beyond (from [GOSIM China 2024](https://china2024.gosim.org/schedules/robrix--a-pure-rust-multi-platform-matrix-client-and-more))
    * Videos: (YouTube link coming soon)
    * Slides:
      [PowerPoint (25MB)](https://github.com/project-robius/files/blob/99bc71ab0eebb0a9ed1aa367253c398ff0622c6f/GOSIM%20China%202024/Robrix%20Talk%20GOSIM%20China%20October%2017%2C%202024.pdf),
      [PDF version (6MB)](https://github.com/project-robius/files/blob/main/GOSIM%20China%202024/Robrix%20Talk%20GOSIM%20China%20October%2017%2C%202024.pdf)
  * Robrix: a Matrix chat client and more (from [GOSIM Europe 2024](https://europe2024.gosim.org/schedule#fediverse))
    * Videos: [YouTube link](https://www.youtube.com/watch?v=P8RGF942A5g)
    * Slides:
      [PowerPoint (22MB)](https://github.com/project-robius/files/raw/3ac0a9d2e9f3c78ea51b4875abe02d288fa3685f/RustNL%202024%20and%20GOSIM%20Europe%202024/Robrix%20Talk%20GOSIM%20Europe%20May%206,%202024.pptx),
      [PDF version (16MB)](https://github.com/project-robius/files/blob/3ac0a9d2e9f3c78ea51b4875abe02d288fa3685f/RustNL%202024%20and%20GOSIM%20Europe%202024/Robrix%20Talk%20GOSIM%20Europe%20May%206%2C%202024.pdf)


The following table shows which host systems can currently be used to build Robrix for which target platforms.
| Host OS | Target Platform | Builds? | Runs? |
| ------- | --------------- | ------- | ----- |
| macOS   | macOS           | ✅      | ✅    |
| macOS   | Android         | ✅      | ✅    |
| macOS   | iOS             | ✅      | ✅    |
| Linux   | Linux           | ✅      | ✅    |
| Linux   | Android         | ✅      | ✅    |
| Windows | Windows         | ✅      | ✅    |
| Windows | Android         | ✅      | ✅    |



## Building and Running

1. First, [install Rust](https://www.rust-lang.org/tools/install).

2. If you're building on **Linux** or **WSL** on Windows, install the required dependencies. Otherwise, proceed to step 3.
   * `openssl`, `clang`/`libclang`, `binfmt`, `Xcursor`/`X11`, `asound`/`pulse`.

   On a Debian-like Linux distro (e.g., Ubuntu), run the following:
   ```sh
   sudo apt-get update
   sudo apt-get install libssl-dev libsqlite3-dev pkg-config binfmt-support libxcursor-dev libx11-dev libasound2-dev libpulse-dev
   ```

3. Then, build and run Robrix (you can optionally add `--release` after `run`):
```sh
cargo run -- 'USERNAME' 'PASSWORD' ['HOMESERVER_URL']
```

* Robrix only supports a standard username + password login currently.
    * Note the usage of **single quotes** (not double quotes), which will prevent your shell from treating certain symbols as globs/regex patterns.
    * If you created your Matrix account using a third-party Single Sign On (SSO) like a Google account, you can set a standard password by using [Element's password reset form](https://app.element.io/#/forgot_password).
* The `HOMESERVER_URL` argument is optional and uses the `"https://matrix-client.matrix.org/"` URL by default.
   * The Matrix homeserver must support Sliding Sync, the same requirement as Element X.


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
- [x] Dynamic backwards pagination based on scroll position/movement: https://github.com/project-robius/robrix/issues/109
- [x] Loading animation while waiting for pagination request: https://github.com/project-robius/robrix/issues/109
- [x] Stable positioning of events during simple timeline update
- [x] Stable positioning of events during complex/multi-part timeline update
- [x] Display simple text-only messages
- [x] Display image messages (PNG, JPEG)
- [x] Rich text formatting for message bodies
- [x] Display reactions (annotations)
- [x] Handle opening links on click
- [x] Linkify plaintext hyperlinks
- [x] Reply previews above messages: https://github.com/project-robius/robrix/issues/82
- [x] Send messages (standalone, no replies)
- [x] Interactive reaction button, send reactions: https://github.com/project-robius/robrix/issues/115
- [x] Reply button, send reply: https://github.com/project-robius/robrix/issues/83
- [ ] Re-spawn timeline as focused on an old event after a full timeline clear: https://github.com/project-robius/robrix/issues/103
- [ ] Display multimedia (audio/video/gif) message events: https://github.com/project-robius/robrix/issues/120
- [ ] Collapsible/expandable view of contiguous "small" events: https://github.com/project-robius/robrix/issues/118
- [x] E2EE device verification, decrypt message content: https://github.com/project-robius/robrix/issues/116

### Auxiliary/admin features: login, registration, settings
- [x] Persistence of app session to disk: https://github.com/project-robius/robrix/issues/112
- [x] Username/password login screen: https://github.com/project-robius/robrix/issues/113
- [ ] SSO, other 3rd-party auth providers login screen: https://github.com/project-robius/robrix/issues/114
- [x] Side panel showing detailed user profile info (click on their Avatar)
- [x] Ignore and unignore users (see known issues)
- [ ] User settings screen
- [ ] Dedicated view of spaces
- [ ] Dedicated view of direct messages (DMs): https://github.com/project-robius/robrix/issues/139
- [ ] Link previews beneath messages: https://github.com/project-robius/robrix/issues/81
- [ ] Keyword filters for the list of all rooms: https://github.com/project-robius/robrix/issues/123
- [ ] Search messages within a room: https://github.com/project-robius/robrix/issues/122
- [ ] Room browser, search for public rooms
- [ ] Room creation
- [ ] Room settings/info screen
- [ ] Room members pane
- [ ] Save/restore events in rooms to/from the event cache upon app shutdown/start: https://github.com/project-robius/robrix/issues/164


## Known problems/issues
 - Matrix-specific links are not yet fully handled (https://matrix.to/...)
 - Ignoring/unignoring a user clears all timelines  (see: https://github.com/matrix-org/matrix-rust-sdk/issues/1703); the timeline will be re-filled using gradual pagination, but the viewport is not maintained
