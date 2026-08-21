# Robrix on the AUR

The AUR is a pile of tiny git repos, one per package, each holding a `PKGBUILD` (the
build recipe) and a `.SRCINFO` (a machine-readable summary of it). Users' tools clone
ours and build from the binary tarball attached to our GitHub release.

`packaging/arch/PKGBUILD.in` is the only file to hand-edit. Everything else is
generated from it:

* `render-pkgbuild.sh` fills in the blanks. `--mode local` for the release build,
  `--mode aur` for the AUR. It asks the release which arch payloads exist and emits
  only those, so aarch64 switches on by itself once we ship one.
* `.SRCINFO` always comes from `makepkg --printsrcinfo`. Never write it by hand; the
  AUR rejects a push where it disagrees with the PKGBUILD.
* `aur-publish.yml` re-renders and pushes on every published release.

Test any change with `./packaging/aur/validate.sh --tag v1.0.0-alpha.2`, which builds
and installs the package in a throwaway Arch container. Needs docker on Linux.

## One-time setup

Nothing below has been done yet. Until step 4, `aur-publish.yml` fails fast on
published releases with a clear error.

**1. Account and key.** Register at <https://aur.archlinux.org> as a project account,
not a personal one, and verify the email. Then:

```sh
ssh-keygen -t ed25519 -N '' -C 'robrix-ci@robius.rs' -f ~/.ssh/robrix-aur
cat ~/.ssh/robrix-aur.pub    # paste into My Account -> SSH Public Key
```

**2. First push.** `aur-publish.yml` refuses to create a new package, so this one is by
hand. Confirm the name is still free, then:

```sh
set -euo pipefail
git ls-remote https://aur.archlinux.org/robrix.git          # expect zero refs
git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/robrix.git /tmp/aur-robrix

./packaging/aur/render-pkgbuild.sh --mode aur --tag v1.0.0-alpha.2 --out /tmp/render
cp /tmp/render/PKGBUILD /tmp/aur-robrix/
cp packaging/aur/aur-repo-LICENSE /tmp/aur-robrix/LICENSE
cd /tmp/aur-robrix
docker run --rm -v /tmp/aur-robrix:/w -w /w archlinux:base-devel bash -c \
  'useradd -m b && chown -R b /w && sudo -u b makepkg --printsrcinfo' > .SRCINFO

git add PKGBUILD .SRCINFO LICENSE
git -c user.name='Project Robius' -c user.email='IT@gosim.org' commit -m 'Add robrix 1.0.0alpha.2-1'
git push origin master
```

**3. Merge request.** A prebuilt `robrix-bin` already exists, unmaintained since
2026-05-29 and two releases behind, so ours is a duplicate under AUR rules until it's
merged. File a Merge request at
<https://aur.archlinux.org/pkgbase/robrix-bin/request> into `robrix`, and comment on
`robrix-bin` so the maintainer sees it. If a Package Maintainer tells you to invert the
merge instead, flip `PKGNAME` in `render-pkgbuild.sh` to `robrix-bin`; that's the whole
change.

**4. CI secret.**

```sh
gh secret set ROBRIX_AUR_SSH_KEY -R project-robius/robrix < ~/.ssh/robrix-aur
```

Use the redirect, not `printf`, so the trailing newline survives. Then add a required
reviewer under Settings -> Environments -> `aur`. That reviewer is the real gate on the
key, since the workflow file itself lives on `main`.

**5. Top-level README.** Once <https://aur.archlinux.org/packages/robrix> resolves, add
to the install section:

```markdown
### Arch Linux

    paru -S robrix     # or: yay -S robrix

Or install the released package directly:

    sudo pacman -U robrix-1.0.0alpha.2-1-x86_64.pkg.tar.zst
```

## Notes

`validate.sh` emits 18 namcap warnings and no `E:`. All expected: 13 are a `--nodeps`
artifact, since namcap can't map soname to package when the deps aren't installed; 4
are the dlopen'd and spawned deps it can't see (`libglvnd`, `dbus`, `xdg-utils`,
`ca-certificates`); 1 is the dynamic linker in its own `DT_NEEDED`.

`pkgver` drops the pre-release hyphen rather than dotting it, because `vercmp` sorts
`1.0.0alpha.2` below `1.0.0` while `1.0.0.alpha.2` and `1.0.0_alpha.2` both sort above,
which would make the real 1.0.0 look like a downgrade.
