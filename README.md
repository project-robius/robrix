# Robrix: a Rust Matrix client built atop [Robius](https://github.com/project-robius)

Robrix is a Matrix chat client written in Rust to demonstrate the functionality of the Robius, a framework for multi-platform application development in Rust.

> ⚠️ Robrix is just getting started and is not yet fully functional.

## Building and Running

[Install Rust](https://www.rust-lang.org/tools/install), and then simply run:
```sh
cargo run -- "USERNAME" "PASSWORD" ["HOMESERVER_URL"]
```

* Robrix only supports a standard username + password login currently.
    * If you created your Matrix account using a third-party Single Sign On (SSO) like a Google account, you can set a standard password by using [Element's password reset form](https://app.element.io/#/forgot_password).
* The `HOMESERVER_URL` argument is optional and uses the `"https://matrix.org"` URL by default.


## Feature status tracker 


- [x] View list of joined rooms
- [x] View timeline of events in a single room
- [ ] Stable positioning of events view during timeline update
- [x] Fetch and display room avatars
- [ ] Fetch and display user avatars
- [x] Backwards pagination (upon viewing a room timeline)
- [ ] Dynamic backwards pagination based on scroll position/movement
- [ ] Loading animation while waiting for pagination request
- [x] Display of simple text-only messages
- [ ] Rich formatting of messages
- [ ] Displaying reactions (annotations)
- [ ] Display multimedia (non-text) message events
- [ ] Inline link previews
- [ ] Inline reply view
- [ ] Reaction button, send reactions
- [ ] Reply button, send reply
- [ ] Collapsible/expandable view of contiguous "small" events
- [ ] Error display: no connection, failure to login, sync timeout.
- [ ] Encrypted rooms, decrypting messages
- [ ] Sending messages

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

