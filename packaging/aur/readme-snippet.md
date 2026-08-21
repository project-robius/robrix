# README.md insertion: the "Installing Robrix" section

Paste-ready text for the repo's top-level `README.md`, plus the exact lines it goes
between. Line numbers are against `README.md` at commit `b348d07b` (299 lines).

> **Do not merge this before the AUR package exists.** Probed 2026-08-20:
> `https://aur.archlinux.org/packages/robrix` returns **404**, and
> `git ls-remote https://aur.archlinux.org/robrix.git` returns zero refs. Add a
> checklist line to `packaging/aur/README.md` step 5 if it helps: *only once
> https://aur.archlinux.org/packages/robrix returns 200, merge this section.*
> Otherwise a reader follows a dead link and `paru -S robrix` answers
> "could not find all required packages".

## Where it goes

Lines 57 through 62 are, verbatim (57, 60 and 61 are blank):

```
                                            <- line 57, blank
> [!IMPORTANT]                              <- line 58
> Robrix only works with Matrix homeservers that support native Sliding Sync, just like other modern clients (e.g., Element X).
                                            <- line 60, blank
                                            <- line 61, blank
## Building & Running Robrix on Desktop     <- line 62
```

**Insert after line 61**, i.e. after the second blank line, and **end the inserted
block with two blank lines** so line 62 keeps the two-blank-line separator the rest of
the file uses between top-level sections. There are only two blank lines in total to
work with, so this is the one placement that leaves the spacing unchanged.

## The section to insert

Everything between the outer fences, exactly:

````markdown
## Installing Robrix

Pre-built packages for every release are on the [releases page](https://github.com/project-robius/robrix/releases/latest):
`.dmg` for macOS, `.exe` for Windows, `.deb` for Debian and Ubuntu, `.AppImage` for any Linux, and `.apk` for Android.

### Arch Linux

Install [`robrix`](https://aur.archlinux.org/packages/robrix) from the AUR with your favorite helper.
We maintain it ourselves, and a new version is pushed automatically on every release.
```sh
paru -S robrix   ## or: yay -S robrix
```

Already have `robrix-bin` installed? Run `paru -S robrix`; pacman will offer to replace it.
An AUR helper won't do that for you on an ordinary `-Syu`, because foreign packages aren't
matched against the new package's `provides`.

Or install the pre-built package straight off the [release page](https://github.com/project-robius/robrix/releases/latest),
with no AUR helper at all. Download the `.pkg.tar.zst` for your architecture, then:
```sh
sudo pacman -U robrix-<pkgver>-1-x86_64.pkg.tar.zst    ## x86_64
sudo pacman -U robrix-<pkgver>-1-aarch64.pkg.tar.zst   ## Arch Linux ARM and Asahi
```

The package version drops the pre-release hyphen, so the `v1.0.0-alpha.2` tag ships
`robrix-1.0.0alpha.2-1-x86_64.pkg.tar.zst`. Releases before we added `aarch64` builds are
x86_64 only, so check the release page for the file before reaching for the second command.

There is also a community-maintained [`robrix-git`](https://aur.archlinux.org/packages/robrix-git)
that builds from source off `main`. It isn't ours, so we can't vouch for it.
````

## Notes on the wording

* The `.pkg.tar.zst` filenames use a `<pkgver>` placeholder rather than a real version.
  Verified 2026-08-20: `v1.0.0-alpha.2` ships **only**
  `robrix-1.0.0alpha.2-1-x86_64.pkg.tar.zst` and `robrix_1.0.0-alpha.2_x86_64.tar.gz`,
  and a ranged GET of the aarch64 payload returns 404, so a copy-pasteable aarch64
  command would name a file that does not exist on any release yet.
* `aarch64` is called out as Arch Linux ARM and Asahi on purpose. Official Arch Linux
  is x86_64-only, so an unqualified "aarch64 supported" would mislead.
* The `robrix-bin` line is there because `provides=` does **not** give existing
  `robrix-bin` users an automatic upgrade path, and nothing can. `robrix-bin` is a
  foreign package: `pacman -Syu` never reads AUR `provides`, `replaces=` would be
  equally inert and the wiki says not to use it in an AUR PKGBUILD, and `paru -Syu`
  looks the name up in the AUR RPC and silently skips it once a merge removes it. An
  explicit `paru -S robrix` is the only thing that triggers the conflict prompt.
* The `robrix-git` sentence stays deliberately neutral. It is actively maintained by
  `dreieck`, but it currently installs no resource tree
  ([issue #929](https://github.com/project-robius/robrix/issues/929)), so pointing
  users at it without a caveat would cost first impressions.

## Two related edits, not included above

Both are outside "add an install section", so they are listed rather than folded in.

1. **`README.md:6-7`** currently reads:

   ```
   > [!TIP]
   > ▶️  [Click here to download & install Robrix](https://github.com/project-robius/robrix/releases/latest) on macOS, iOS, Android, Linux, and Windows.
   ```

   It could point at the new `## Installing Robrix` section instead of straight at the
   releases page.

2. **`README.md:70-77`** gives Linux from-source build dependencies for Debian-likes
   only, with no `pacman` equivalent. The Arch line would be:

   ```sh
   sudo pacman -S --needed base-devel cmake clang openssl sqlite libx11 libxcursor libxkbcommon wayland alsa-lib libpulse libglvnd
   ```

   Note this is the **build** dependency list, which is not the PKGBUILD's runtime
   `depends`. A from-source Arch build also hits the `aws-lc-rs`/`jitterentropy` issue
   from [#889](https://github.com/project-robius/robrix/issues/889) if `CFLAGS` carries
   an optimisation level.
