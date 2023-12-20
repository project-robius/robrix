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


