# Robrix on the AUR: operator runbook

Everything needed to stand up, publish and keep running the official Robrix AUR
package. Read it once before touching the AUR; several steps have two-week clocks
attached.

> **Sourcing note.** Claims about aurweb internals, account registration status and
> past request history could not be checked from a script: `aur.archlinux.org`'s web
> UI, its cgit and `gitlab.archlinux.org` all sit behind an Anubis proof-of-work
> challenge. Everything reachable over the RPC API, plain git, the wiki REST API and
> the GitHub API **was** verified on 2026-08-20 and is marked as such. Confirm the
> rest in a browser before planning a timeline around it.

## Contents

1. [What we publish, and the name decision](#1-what-we-publish-and-the-name-decision)
2. [How the pieces fit together](#2-how-the-pieces-fit-together)
3. [Create the AUR account and the CI SSH key](#3-create-the-aur-account-and-the-ci-ssh-key)
4. [Validate locally](#4-validate-locally)
5. [The first push, which creates `robrix`](#5-the-first-push-which-creates-robrix)
6. [The merge request: `robrix-bin` into `robrix`](#6-the-merge-request-robrix-bin-into-robrix)
7. [Add the SSH key as a repo secret](#7-add-the-ssh-key-as-a-repo-secret)
8. [How the automation keeps it current](#8-how-the-automation-keeps-it-current)
9. [How aarch64 turns itself on](#9-how-aarch64-turns-itself-on)
10. [Switching to `robrix-bin` (one line)](#10-switching-to-robrix-bin-one-line)
11. [Decisions worth not re-litigating](#11-decisions-worth-not-re-litigating)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. What we publish, and the name decision

We publish the **plain name `robrix`**, carrying the prebuilt payload tarball from
our own GitHub release. Verified 2026-08-20: the AUR has no `robrix` pkgbase (RPC
`info` returns only `robrix-bin` and `robrix-git`), and
`git ls-remote https://aur.archlinux.org/robrix.git` returns zero refs, so there is
not even a deleted package's history to inherit.

### The rules this bends, stated plainly

Quoted verbatim from [AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines),
fetched from the wiki REST API:

> Packages that use **prebuilt** deliverables, when the sources are available, must
> use the `-bin` suffix.

> **Check the AUR** if the package **already exists**. [...] Do not create duplicate
> packages.

Both sit under a Warning box reading *"Packages that violate the rules may be deleted
without warning"*. Robrix's sources are public and MIT, so the Java exception does not
apply. This was chosen with that risk understood. The mitigations exist to make the
situation resolvable rather than adversarial:

| Mitigation | Where |
| --- | --- |
| `provides=("robrix-bin=${pkgver}")`, so `paru -S robrix-bin` keeps resolving after a merge | `packaging/aur/render-pkgbuild.sh` |
| `conflicts=('robrix-bin' 'robrix-git')`, so an install offers a clean replacement prompt instead of a raw file conflict on `/usr/bin/robrix` | same |
| An AUR **merge request** filed against `robrix-bin` the same day | [step 6](#6-the-merge-request-robrix-bin-into-robrix) |
| Switching to `robrix-bin` is genuinely one line | [step 10](#10-switching-to-robrix-bin-one-line) |

If a Package Maintainer rejects the merge and asks for a rename, do step 10 and move
on. Do not argue the point.

### What `provides` does and does not buy

Be precise about this, because it is easy to overstate and the merge request should
not overstate it.

`robrix-bin` on a user's machine is a **foreign** package. `pacman -Syu` only walks
sync databases, so it never looks at `provides` from the AUR and will never offer to
replace it. `replaces=` would be equally inert, and the wiki says not to use it in an
AUR PKGBUILD anyway. `paru -Syu` looks `robrix-bin` up by name in the AUR RPC; once a
merge lands, that name is gone and the helper just skips it.

What `provides` genuinely does: after a merge, `paru -S robrix-bin` resolves to
`robrix`, and anything declaring `depends=('robrix-bin')` keeps working. The conflict
prompt only fires on an explicit `paru -S robrix`. That is why
`packaging/aur/readme-snippet.md` tells existing `robrix-bin` users to run
`paru -S robrix` by hand.

### The other two packages

* **`robrix-bin`** (maintainer `mmoya`). Verified over the RPC API: version
  `0.0.1.pre.alpha.4-2`, first submitted 2025-02-03, last modified 2025-10-06,
  out-of-date since 2026-05-29, 0 votes, 0 popularity. Its PKGBUILD has no `depends`,
  no `license`, no `provides` and no `conflicts`.
* **`robrix-git`** (maintainer `dreieck`) builds from `main` and already declares
  `conflicts=('robrix' 'robrix-bin')` and `provides=('robrix=$pkgver')`. Treat him as
  an ally. Worth telling him regardless of the AUR question:
  [issue #929](https://github.com/project-robius/robrix/issues/929) reports
  `robrix-git` installing no resource tree, so its users get a fontless GUI.

Do not file a **deletion** request against `robrix-bin`. The reviewers report one was
already filed and rejected with "Not a reason for deletion"; that specific history is
in the unverifiable set above, but the wiki is explicit that deletion requests get
rejected in favour of disowning, so the advice stands either way.

---

## 2. How the pieces fit together

| File | Role |
| --- | --- |
| `packaging/arch/PKGBUILD.in` | The **only** copy of the package metadata. Placeholders: `@PKGNAME@ @PKGVER@ @PKGREL@ @ARCH@ @URL@ @PROVIDES@ @CONFLICTS@ @SOURCES@ @LICENSE_DIR_FIXUP@` |
| `packaging/aur/render-pkgbuild.sh` | Fills them in two ways. `--mode local` for the release build (bare filenames, `SKIP` sums), `--mode aur` for the AUR (release URLs, real sha256). Holds the one-line `PKGNAME` switch. |
| `packaging/aur/PKGBUILD`, `.SRCINFO` | The rendered pair for the release currently on the AUR, committed so it is reviewable and diffable. Kept current by the `sync` job in `aur-publish.yml`; never hand-edit them. |
| `packaging/aur/validate.sh` | Renders, builds, lints, installs and `ldd`s the package in an Arch container, both arches. |
| `.github/workflows/aur-publish.yml` | Renders, test-builds, and pushes to the AUR on `release: published`. |
| `packaging/aur/release-workflow-changes.md` | The paste-ready `release.yml` edits for dual-arch packaging. |

The dedup requirement is met by construction: there is exactly one template and one
renderer, and the two renderings differ **only** in the `@SOURCES@` block. Confirm it
yourself:

```sh
set -euo pipefail
./packaging/aur/render-pkgbuild.sh --mode local --srcver 1.0.0-alpha.2 --arch x86_64 --out /tmp/a
diff /tmp/a/PKGBUILD packaging/aur/PKGBUILD
```

The only hunks are `@SOURCES@`'s three lines. Everything else, `pkgdesc`, `depends`,
`license`, `options`, `package()`, is byte-identical because it comes from the one
template.

---

## 3. Create the AUR account and the CI SSH key

### 3.1 A project-owned AUR account

Register at <https://aur.archlinux.org/register> with a **project** identity, not a
personal one (suggested `projectrobius`, `contact@robius.rs`). Tying the package to an
individual recreates the bus-factor problem `robrix-bin` is stuck in.

Two things to check before planning around any date, both in the unverifiable set:
new-account registration may still be closed following the aurweb 6.5.0 deployment,
and every mutating SSH action is reported to require a verified email, with unverified
accounts warned at 7 days and deleted at 14. Load the register page and your account
page in a browser and see for yourself.

### 3.2 A dedicated CI key

An AUR SSH key grants push access to **every** package the account maintains. Never
reuse a personal key.

```sh
set -euo pipefail
# No passphrase: CI cannot answer a prompt, and the workflow validates this.
ssh-keygen -t ed25519 -N '' -C 'robrix-ci@robius.rs' -f ~/.ssh/robrix-aur
chmod 600 ~/.ssh/robrix-aur
cat ~/.ssh/robrix-aur.pub
```

Paste the `.pub` contents into **My Account -> SSH Public Key**. Multiple keys go in
that one field separated by newlines, so a human key can sit alongside the CI key.

Local git config for the manual steps below, guarded so re-running section 3 does not
append a second stanza that ssh would then ignore:

```sh
set -euo pipefail
install -d -m 700 ~/.ssh
touch ~/.ssh/config
grep -q '^Host aur.archlinux.org' ~/.ssh/config || cat >> ~/.ssh/config <<'EOF'

Host aur.archlinux.org
  User aur
  IdentityFile ~/.ssh/robrix-aur
  IdentitiesOnly yes
EOF
```

### 3.3 Host keys

`aur-publish.yml` pins all three `aur.archlinux.org` host keys rather than running
`ssh-keyscan` at job time. Verified two ways on 2026-08-20, a live `ssh-keyscan` and
the announcement at <https://archlinux.org/news/aur-migration-new-ssh-hostkeys/>, and
the three key blobs in the workflow are byte-identical to the scan:

| Type | Fingerprint |
| --- | --- |
| ED25519 | `SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4` |
| ECDSA | `SHA256:uTa/0PndEgPZTf76e1DFqXKJEXKsn7m9ivhLQtzGOCI` |
| RSA | `SHA256:5s5cIyReIfNNVGRFdDbe3hdYiI5OelHGpw2rOUud3Q8` |

Do the same locally the first time, and compare before accepting:

```sh
set -euo pipefail
ssh-keyscan -t rsa,ecdsa,ed25519 aur.archlinux.org > /tmp/aur_hostkeys
ssh-keygen -lf /tmp/aur_hostkeys        # compare against the table above
cat /tmp/aur_hostkeys >> ~/.ssh/known_hosts   # only once they match
```

---

## 4. Validate locally

### 4.1 Anywhere, no container

The renderer is plain bash 3.2, so it runs on stock macOS as well as Linux.

```sh
set -euo pipefail
cd /path/to/robrix
./packaging/aur/render-pkgbuild.sh --mode aur --tag v1.0.0-alpha.2 --out /tmp/robrix-aur
bash -n /tmp/robrix-aur/PKGBUILD
bash -c 'set -eu; . /tmp/robrix-aur/PKGBUILD
  echo "$pkgname $pkgver-$pkgrel  arch=${arch[*]}"
  echo "provides=${provides[*]}  conflicts=${conflicts[*]}"
  echo "${source_x86_64[*]}"'
```

Against `v1.0.0-alpha.2` you get `arch=('x86_64')` and the log line
`no robrix_1.0.0-alpha.2_aarch64.tar.gz on v1.0.0-alpha.2, leaving that arch out`.
That is correct: the release predates the dual-arch CI change.

### 4.2 The full check, on the x86_64 Ubuntu box with docker

```sh
set -euo pipefail
./packaging/aur/validate.sh --tag v1.0.0-alpha.2
```

It renders once, then per arch runs `makepkg`, `namcap` on both the PKGBUILD and the
built package, `pacman -U` so the real dependency resolution happens, and `ldd` on the
installed binary, and prints a PASS/FAIL summary. Today it passes x86_64 and skips
aarch64 with an explicit message, because there is no aarch64 payload to build.

Once a release does ship one, `--with-binfmt` registers qemu so the aarch64 leg runs
on the same x86_64 box:

```sh
./packaging/aur/validate.sh --tag v1.0.0-alpha.3 --with-binfmt
```

Be aware what that flag does: it runs `tonistiigi/binfmt` with `--privileged`, which
writes **system-wide** `binfmt_misc` registrations, replaces any existing
qemu-user-static handlers on that host, and persists until reboot. The script prints
`ls /proc/sys/fs/binfmt_misc/` first so you can see what you are replacing. Skip the
flag on a machine whose cross-build tooling you care about.

Image choice, verified against the Docker registry on 2026-08-20:
`library/archlinux:base-devel` publishes an **amd64 manifest only**, while
`menci/archlinuxarm:base-devel` publishes amd64, arm64, armv7 and riscv64 and was
updated the same day. That is why aarch64 uses Arch Linux ARM, and why `validate.sh`
always passes an explicit `--platform` rather than letting docker pick.

CI itself needs none of this: `ubuntu-22.04` and `ubuntu-22.04-arm` are both native
runners.

---

## 5. The first push, which creates `robrix`

The automation deliberately **refuses** to create a new pkgbase. An AUR clone succeeds
even for a name that does not exist, so a typo'd `PKGNAME` would silently publish a
brand-new package; the workflow checks for a `PKGBUILD` and bails when there is none.
So the first push is a human job.

```sh
set -euo pipefail
cd /path/to/robrix

# Validate first (section 4). Do not skip this.
./packaging/aur/validate.sh --tag v1.0.0-alpha.2

# Confirm the name is still free before creating anything.
git ls-remote https://aur.archlinux.org/robrix.git   # expect exit 0, zero refs

git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/robrix.git /tmp/aur-robrix
# "warning: You appear to have cloned an empty repository." is expected.

cp packaging/aur/PKGBUILD packaging/aur/.SRCINFO /tmp/aur-robrix/
cp packaging/aur/aur-repo-LICENSE /tmp/aur-robrix/LICENSE
cd /tmp/aur-robrix

# Checks the AUR server enforces, done before the push rather than after.
test -f PKGBUILD && test -f .SRCINFO && test -f LICENSE
grep -qx 'pkgbase = robrix' .SRCINFO
find . -mindepth 1 -maxdepth 1 -type d ! -name .git     # must print nothing
find . -maxdepth 1 -type f -size +250k ! -name '.git*'  # must print nothing

git add PKGBUILD .SRCINFO LICENSE
git -c user.name='Project Robius' -c user.email='contact@robius.rs' \
  commit -m 'robrix 1.0.0alpha.2-1: official upstream binary package'
git push origin master
```

Confirm it landed: <https://aur.archlinux.org/packages/robrix>

**About that `LICENSE` file.** It is the package **sources** license, not Robrix's. The
submission rules list it, and the note attached says packages without it, or with
anything other than 0BSD, are *not eligible* for promotion to the official repos. The
text is Arch's own, copied from
`gitlab.archlinux.org/archlinux/devtools/-/blob/master/data/LICENSE`, and lives here as
`packaging/aur/aur-repo-LICENSE`. A root-level file is fine: the AUR hook restricts
only *subdirectories* (to `keys/` and `LICENSES/`), and the 250 KiB cap is nowhere
near. Do **not** add a `LICENSES/` directory casually, since its presence makes the
server run `pkgctl license check` and reject the push on any REUSE non-compliance.

**Never commit the payload tarball or the built package.** The guidelines forbid it and
the hook rejects any root-level file over 250 KiB anyway.

---

## 6. The merge request: `robrix-bin` into `robrix`

File it **the same day** as step 5. A duplicate that sits unaddressed looks like
squatting; a duplicate with an open merge request looks like a handover in progress.

**Expect pushback.** The wiki defines Merge as *"Request to delete a pkgbase and
transfer its votes and comments to another pkgbase"* and frames it as the action for
an upstream project **rename**, which this is not. It also notes that when a package
has no votes or comments, *"a deletion request linking to the new package is
identical"*, and `robrix-bin` has 0 votes and 0 popularity. So the only thing actually
transferred is its comment thread; the request's real value is as a good-faith signal.
Filing it also puts a Package Maintainer's eyes directly on the fact that the merge
**target** is a prebuilt package without the `-bin` suffix. Realistic outcomes:
acceptance, rejection, or an instruction to invert the merge. If it is the third, go
straight to [step 10](#10-switching-to-robrix-bin-one-line).

### 6.1 Contact the maintainer first

Soft-required by the wiki, and it is what a Package Maintainer will look for. Do both:
comment on <https://aur.archlinux.org/packages/robrix-bin>, and email
`mmoya@mmoya.org`.

> Hi Maykel, I'm from Project Robius, upstream for Robrix. We've started publishing an
> official binary package straight from our release CI, at
> https://aur.archlinux.org/packages/robrix. It carries the same upstream payload you
> package, plus the `depends`, `license`, `provides` and `conflicts` metadata, and it
> updates automatically on every release.
>
> `robrix-bin` has been flagged out-of-date since 2026-05-29 and is two releases
> behind, so rather than leave two binary packages side by side I've filed a merge
> request into `robrix`. If you'd rather keep maintaining it, say so and I'll withdraw
> the request. Either way, thank you for packaging Robrix first. You're credited as
> `# Contributor:` in ours; say the word and I'll remove it.
>
> Also worth flagging: dreieck's request to add `provides=("robrix=${pkgver}")` and
> `conflicts=("robrix")` is still open, and without it users hit a raw file conflict on
> `/usr/bin/robrix`.

### 6.2 File it

**<https://aur.archlinux.org/pkgbase/robrix-bin/request>**

Note: no trailing slash. Probed 2026-08-20, the trailing-slash form returns a 307 to
plain **http://**, a needless protocol downgrade on the one flow carrying your session
cookie. The slashless form returns 303 to the login page, which is the route working.

* **Type:** `Merge`
* **Merge into:** `robrix`
* **Comments:**

> `robrix-bin` is superseded by `robrix`, which is maintained by upstream (Project
> Robius, the Robrix developers) and pushed automatically from our release CI on every
> published release.
>
> `robrix` ships the same upstream prebuilt payload that `robrix-bin` ships, and adds
> the metadata `robrix-bin` has none of: a full `depends` array (including the
> dlopen'd `libglvnd` and `dbus` and the spawned `xdg-utils`, which no static analysis
> can detect), an SPDX `license` array with the license files installed,
> `options=('!strip' '!debug')`, and `provides=("robrix-bin=${pkgver}")` plus
> `conflicts=('robrix-bin')` so anyone installing it over `robrix-bin` gets a clean
> replacement prompt rather than a file conflict on `/usr/bin/robrix`. It tracks both
> x86_64 and aarch64 as our release CI publishes them.
>
> `robrix-bin` is at 0.0.1.pre.alpha.4-2, last updated 2025-10-06, flagged out-of-date
> since 2026-05-29, and two upstream releases behind (v1.0.0-alpha.1 and v1.0.0-alpha.2
> have shipped since). It has 0 votes and 0 popularity. Its maintainer mmoya has been
> contacted by email and in a comment on the package page, and is credited as
> `# Contributor:` in `robrix`.
>
> Upstream contact: contact@robius.rs, https://github.com/project-robius/robrix

---

## 7. Add the SSH key as a repo secret

```sh
set -euo pipefail
# `< file` preserves the trailing newline OpenSSH needs. Never pipe it through
# printf or echo -n.
gh secret set ROBRIX_AUR_SSH_KEY -R project-robius/robrix < ~/.ssh/robrix-aur
gh secret list -R project-robius/robrix | grep ROBRIX_AUR_SSH_KEY
```

Optionally wire it into the repo's secret helpers. In
`packaging/release-secrets.env.example`, after the `ROBRIX_RELEASE` line:

```sh
# --- AUR publishing (Arch Linux) ---
AUR_SSH_KEY_FILE=''                     # path to the CI-only, passphrase-less ed25519 key
```

and in `packaging/upload-release-secrets.sh`, after the `-- Release token --` block:

```sh
echo "-- AUR publishing --"
set_file ROBRIX_AUR_SSH_KEY "${AUR_SSH_KEY_FILE:-}"
```

`set_file`, never `set_str`: `set_str` does `printf '%s'` and strips the trailing
newline OpenSSH requires.

### The `aur` environment

The `push` job declares `environment: aur`. GitHub creates it on first run with no
protection rules, so nothing breaks if you do nothing, but **add a required reviewer**
under Settings -> Environments -> aur. That is the real gate here. The key is written
only inside that job, and that job runs no script from the repo, but the workflow file
itself lives on `main`, so anyone who can land a commit there can change what runs
next to the key. A required reviewer is what turns that into a two-person action, and
it matches the "publish it by hand" philosophy `release.yml` already uses.

### Optional: `ROBRIX_AUR_SYNC_TOKEN`

The `sync` job commits the rendered `packaging/aur/{PKGBUILD,.SRCINFO}` back to `main`
so the in-repo copy never goes stale. It tries `GITHUB_TOKEN` first, which a protected
`main` may reject exactly as it does for `licenses.yml`. If it does, set
`ROBRIX_AUR_SYNC_TOKEN` to an admin PAT with `contents: write` and the job picks it up.
The job is `continue-on-error`, so a rejection is a warning, never a failed release.

---

## 8. How the automation keeps it current

`.github/workflows/aur-publish.yml` fires on `release: types: [published]`, plus a
`workflow_dispatch` with a `tag` input for backfills and reruns. Three jobs:

**`render`** holds no secrets, because it is the job that runs repo-controlled shell
and a container. It checks out `main`, clones the AUR repo **read-only over https** to
see where the package stands, settles `pkgrel`, renders, and then in
`archlinux:base-devel` runs a `vercmp` downgrade guard, `makepkg --printsrcinfo`, a
real `makepkg` build against the already-downloaded payloads, and `namcap`. It uploads
the pair as an artifact.

**`push`** is the only job with the AUR key. It downloads the artifact, writes the key
at mode 0600, pins the three host keys, clones over ssh, copies two files, commits and
plain-pushes.

**`sync`** commits the same pair back to `main`.

Properties worth knowing:

* **Idempotent.** The comparison covers **both** `PKGBUILD` and `.SRCINFO`. A matching
  PKGBUILD next to a stale `.SRCINFO` is exactly the state that leaves every helper
  showing the old version, so it is repaired rather than skipped.
* **`pkgrel` is right in both directions.** A new `pkgver` resets it to 1. An
  unchanged `pkgver` with changed content bumps it, so users actually see the update.
  It never ratchets across releases.
* **No downgrades.** `vercmp` compares the new `pkgver` against the AUR's and hard-fails
  if it would go backwards, which is what protects you from dispatching an old tag.
* **No forced pushes.** A rejected push means somebody else pushed, and a human should
  look.
* **A no-op rerun is free at the git level but not at the network level.** The payloads
  are downloaded before the comparison can happen, so a redundant rerun still costs
  ~70 MB and a few minutes. The job has `timeout-minutes: 45`.

### Prereleases are not skipped, on purpose

Every Robrix release so far *is* a pre-release version string (`1.0.0-alpha.N`), so
skipping them would mean the AUR package never updates. The GitHub "prerelease"
checkbox is also unreliable here: `release.yml` computes it from the tag containing a
hyphen, but a human editing the draft can flip it, and `v1.0.0-alpha.2` is in fact
published with `isPrerelease: false` despite the hyphen (verified via the GitHub API).

On the trigger itself: `published` fires for drafts, prereleases and full releases
alike, which is why it is the right choice. `released` also fires when a draft is
published, but skips prereleases, so switching to it would silently stop publishing
every `1.0.0-alpha.N`.

### Fixing a packaging bug without an app release

Edit `packaging/arch/PKGBUILD.in` or the renderer, merge to `main`, then:

```sh
gh workflow run aur-publish.yml -R project-robius/robrix -f tag=v1.0.0-alpha.2
```

The `pkgrel` bump is automatic.

The wiki has a standing caution about exactly this setup, worth re-reading once a year:
*"Automation is a valuable tool for maintainers, but it can not replace manual
intervention (e.g. projects can change license, add or remove dependencies [...]).
Automated PKGBUILD updates are used at your own risk and any malfunctioning accounts
and their packages may be removed without prior notice."* Read the rendered diff on
each release rather than assuming.

---

## 9. How aarch64 turns itself on

Nothing to flip. The renderer probes the release for `robrix_<version>_<arch>.tar.gz`
for each of `x86_64` and `aarch64` with a one-byte ranged GET, and emits `arch=()`,
`source_<arch>=()` and `sha256sums_<arch>=()` for only the ones that answer.

* **Today** `v1.0.0-alpha.2` has only an x86_64 payload (verified: the x86_64 URL
  returns 206, the aarch64 URL 404), so the rendered PKGBUILD is `arch=('x86_64')`. An
  Arch ARM user gets a clean "not available for your architecture" instead of a 404
  mid-build.
* **The first release built with the `release.yml` changes** also uploads
  `robrix_<version>_aarch64.tar.gz`, and the next AUR run renders
  `arch=('x86_64' 'aarch64')` with both source blocks. No edit, no flag.

**The aarch64 path is not theoretical, and it is a regression fix.** Robrix
*used* to ship an aarch64 pacman payload: `v0.0.1-pre-alpha-4` carries both
`robrix_0.0.1-pre-alpha-4_x86_64.tar.gz` and `robrix_0.0.1-pre-alpha-4_aarch64.tar.gz`.
Rendering against that tag today produces exactly what a future dual-arch release
will, and it was used to exercise the whole path end to end:

```
arch=('x86_64' 'aarch64')
_srcver=0.0.1-pre-alpha-4
_baseurl="https://github.com/project-robius/robrix/releases/download/v${_srcver}"

source_x86_64=("${_baseurl}/robrix_${_srcver}_x86_64.tar.gz")
sha256sums_x86_64=('26b0488cf809396179bffaa423aef701e25d9089265e240851c4c71a73e0c64f')

source_aarch64=("${_baseurl}/robrix_${_srcver}_aarch64.tar.gz")
sha256sums_aarch64=('240e0154c732ded108686c4b6014066f4da4ceaf563ed44e6b976fd9481eba5e')
```

Both checksums are of bytes actually downloaded from those public URLs. `v1.0.0-alpha.1`
then shipped a single differently-named `Robrix-1.0.0-alpha.1-Arch-Linux-pacman.tar.gz`,
and `v1.0.0-alpha.2` shipped x86_64 only, so the arch was lost along the way rather than
never having existed. `release.yml` change (a) puts it back.

`makepkg --printsrcinfo` implies `--ignorearch` (verified in `makepkg.sh.in`, the
`--printsrcinfo` case sets `IGNOREARCH=1`) and `srcinfo.sh.in` loops over every
declared arch, so a `.SRCINFO` generated in the x86_64 container carries the complete
dual-arch metadata.

To add a third arch later, append it to `CANDIDATE_ARCHES` in the renderer and add a
CI leg that uploads the payload.

Who this serves: official Arch Linux is x86_64-only (the wiki's `arch` section says so
outright), so the aarch64 package is for Arch Linux ARM and Arch-on-Asahi, separate
projects. All 14 `depends` and both `optdepends` were checked against Arch Linux ARM's
own `core.db` and `extra.db` for aarch64 on 2026-08-20 and every one exists there.

---

## 10. Switching to `robrix-bin` (one line)

In `packaging/aur/render-pkgbuild.sh`:

```sh
PKGNAME='robrix'      ->      PKGNAME='robrix-bin'
```

That is the entire change, and it was verified by rendering both ways:

| `PKGNAME` | `provides` | `conflicts` | `package()` |
| --- | --- | --- | --- |
| `robrix` | `("robrix-bin=${pkgver}")` | `('robrix-bin' 'robrix-git')` | `cp -a` only |
| `robrix-bin` | `("robrix=${pkgver}")` | `('robrix' 'robrix-git')` | `cp -a` plus an `mv` of the license dir |

The license-dir rename matters and is handled: the payload names it after the app, and
namcap looks for it under `$pkgname`. The renderer emits that `mv` line **only** when
the two differ, so neither rendering carries a dead branch. Same for `provides` and
`conflicts`: they are computed in the renderer, so the published PKGBUILD is flat.

Then, in order:

1. Get push rights on `robrix-bin` first. It is not orphaned, so you need `mmoya` to
   co-maintain or disown, or an orphan request granted. **Multi-week**, see below.
2. Merge the one-line change to `main`.
3. Push once by hand (step 5, against `robrix-bin.git`), then let the workflow take
   over. It reads the name from `--print-pkgname`, so nothing else needs editing.
4. File a merge request the other way, `robrix` into `robrix-bin`.

Two things that stay put:

* `/usr/lib/robrix/` on disk. Makepad resolves resources from the executable path plus
  the app name, not the pacman package name.
* The `.pkg.tar.zst` release asset name is not independent. `PKGNAME` drives both
  renderings, so flipping it renames that asset too.

### Timeline if mmoya does not answer

`robrix-bin` was flagged out-of-date on 2026-05-29. From the wiki's Requests section
(verified): orphan requests are *"usually made after it has been flagged out-of-date
for two weeks"*, *"only granted after a two week cooldown if the current maintainer did
not react"*, and *"for packages flagged for at least 180 days, orphan requests are
automatically accepted"*.

| Step | Earliest |
| --- | --- |
| Contact mmoya | now |
| File an **orphan** request (not deletion) | now; the 2-week flag prerequisite passed long ago |
| A Package Maintainer may grant it | filing date + 14 days |
| Automatic acceptance | 2026-11-25 (flag + 180 days) |

The reviewers additionally report that since aurweb 6.5.0 you cannot self-adopt, and
that unhandled adoption requests auto-**reject** after 14 days. That is in the
unverifiable set; budget for two Package Maintainer interactions and consider a
courteous heads-up on `aur-general` either way.

---

## 11. Decisions worth not re-litigating

**Soname `depends` were considered and rejected.** For a prebuilt binary,
`depends=('libssl.so=3-64')` is the usual argument: it blocks an openssl soname bump
until a rebuilt package lands, instead of letting the app die at exec. It does not work
here. Arch Linux ARM's `openssl` and `libgcc` declare **no** `%PROVIDES%` at all
(checked directly in ALARM's `core.db`), while Arch x86_64's openssl provides
`libcrypto.so=3-64` and `libssl.so=3-64`. A versioned soname dep would make the package
flatly uninstallable on the exact platform aarch64 support exists for. It is also not
what comparable AUR binary packages do: `visual-studio-code-bin`, `zoom` and
`slack-desktop` all use plain package names. The residual risk is real and the answer
to it is a prompt release after any soname bump.

**`libgcc`, not `gcc-libs`.** `gcc-libs` is an empty metapackage (installed size 0 on
archlinux.org); `libgcc` is the package that owns `libgcc_s.so.1` and declares
`provides=['libgcc_s.so=1-64']`. Both exist on ALARM aarch64.

**The `license` array is bounded by the number of license files.** namcap's
`licensepkg` rule errors with `license-file-missing` when the number of *uncommon*
SPDX symbols exceeds the number of files in `/usr/share/licenses/$pkgname/`. Arch's
`licenses` package ships 100 common texts; `Apache-2.0`, `CC-BY-3.0` and `GPL-2.0-only`
are among them, `MIT`, `OFL-1.1`, `Font-exception-2.0` and every `LicenseRef-*` are
not. The payload ships exactly two license files, so `license=('MIT' 'Apache-2.0'
'OFL-1.1' 'CC-BY-3.0')` is the largest honest array that stays namcap-clean. The
package's own `copyright` file enumerates all eight licenses per file with full texts,
which is where a reviewer should look. Adding an id here means adding a file too.

**`LICENSE-MIT` is deliberately gone from the package.** The previous PKGBUILD did
`install -Dm644 "${srcdir}/LICENSE-MIT" ...`, so the shipped `.pkg.tar.zst` has it.
The payload's own `usr/share/licenses/robrix/copyright` contains the complete MIT text
verbatim, so the separate file was a duplicate, and dropping it removes the only base
`source=` entry, which in turn removes a `SRCDEST` collision hazard (a shared
`SRCDEST` would happily hand makepkg some other package's cached `LICENSE-MIT`). A
`pacman -Syu` will delete that one file on upgrade; that is intended, and the release
workflow's `cp` drops it too.

**`robrix-git`'s `provides` vercmp-sorts above ours.** `robrix-git` declares
`provides=('robrix=1.0.0.alpha.1.r2741.20260605.7291b335')`, which compares **greater**
than `1.0.0alpha.2`: at the divergence point one side consumes a `.` separator and the
other consumes none, and the unequal separator run length decides it before
`alpha.1` vs `alpha.2` is ever reached. Nothing breaks today because the two conflict
outright. Just never rely on a versioned `depends=('robrix>=...')` to exclude it; rely
on `conflicts`.

**The binary is already stripped.** Verified by parsing the shipped ELF: no `.symtab`,
no `.debug_*`, no `.gnu_debuglink` (`Cargo.toml` sets `strip`). `options=('!strip'
'!debug')` stays anyway, so makepkg never rewrites a vendor artifact and never tries to
split a debug package.

**One known payload wart, not fixed here.**
`usr/lib/robrix/makepad_widgets/resources/LiberationMono-Regular.ttf` ships mode 0755;
it and `/usr/bin/robrix` are the only executable files in the tarball. namcap's
`permissions` rule does not flag it, and it originates in the payload, so fixing it in
`package()` would diverge the pacman package from the `.deb` and `.AppImage`. Fix it
upstream in the packaging pipeline instead.

---

## 12. Troubleshooting

### `release <tag> ships no robrix_<ver>_<arch>.tar.gz payload`

The release is still a draft; draft assets 404 for anonymous downloads, which is what
the renderer sees. Publish it, then re-run:

```sh
set -euo pipefail
TAG='v1.0.0-alpha.2'
gh release view "$TAG" -R project-robius/robrix --json isDraft,assets \
  --jq '{draft:.isDraft, assets:[.assets[].name]}'
gh workflow run aur-publish.yml -R project-robius/robrix -f "tag=$TAG"
```

### `... has no PKGBUILD. This workflow only updates an existing package`

Either nobody has done the manual first push (step 5), or `PKGNAME` does not match a
real AUR package. AUR clones succeed for names that do not exist, which is why this
guard exists rather than relying on the clone failing.

### Push rejected: `<pkgbase> is orphaned`

Run `ssh aur@aur.archlinux.org adopt <pkgbase>` to file an adoption request and wait
for a Package Maintainer to grant it.

### Push rejected on email or authentication

Reported to require a verified email since aurweb 6.5.0. Check **My Account**. Test the
key in isolation:

```sh
ssh -i ~/.ssh/robrix-aur -o IdentitiesOnly=yes aur@aur.archlinux.org help
```

### Push rejected: `missing .SRCINFO` / `must not contain subdirectories`

Server-side rules. `.SRCINFO` must be in the HEAD commit, `PKGBUILD` in every commit in
the pushed range, only `keys/` and `LICENSES/` subdirectories are allowed, no
root-level file may exceed 250 KiB, and the branch must be `master`.

### Push rejected: non-fast-forward

Somebody pushed outside CI. Do not overwrite it. Look first:

```sh
git -C /tmp/aur-robrix fetch origin
git -C /tmp/aur-robrix log --oneline HEAD..origin/master
git -C /tmp/aur-robrix diff HEAD origin/master
```

### Host key verification failed

Re-scan and compare against
<https://archlinux.org/news/aur-migration-new-ssh-hostkeys/> before changing anything:

```sh
ssh-keyscan -t rsa,ecdsa,ed25519 aur.archlinux.org | ssh-keygen -lf -
```

If they differ and Arch has announced a rotation, update the `known_hosts` heredoc in
`aur-publish.yml`. If they differ and Arch has announced nothing, stop and investigate.
That is what pinning is for.

### namcap warnings

Expect `dependency-not-needed` for `libglvnd`, `dbus`, `xdg-utils` and
`ca-certificates`: namcap reads ELF sonames only, so it cannot see
`dlopen("libEGL.so.1")` from Makepad, `dlopen("libdbus-1.so.3")` from the file picker,
a spawned `xdg-open`, or `SSL_CTX_set_default_verify_paths()` reading `/etc/ssl/certs`.
Dropping any of them reintroduces the "can't load LibEGL" class of failure.

That list is derived from namcap's rules, **not** from a run against this package,
because no Arch machine was available. Regenerate it from the first real
`validate.sh` or CI output and keep this paragraph in sync. Any `E:` is worth blocking
on; `validate.sh` already fails the run when it sees one.

### `robrix is not available for the '<arch>' architecture`

`makepkg` refuses because the rendered `arch=()` does not include the container's
`CARCH`. Testing aarch64 against a release with no aarch64 payload gives exactly this,
and it is the renderer working correctly. See section 9.

### The AUR web UI will not load from a script

`aur.archlinux.org`'s web UI, cgit and snapshot tarballs sit behind an Anubis
proof-of-work challenge, as does `wiki.archlinux.org`'s UI. Three things do work
unauthenticated:

* the RPC API: `https://aur.archlinux.org/rpc/v5/info?arg[]=robrix`
* plain git over `https://aur.archlinux.org/<pkgbase>.git`
* the wiki REST API: `https://wiki.archlinux.org/rest.php/v1/page/<Page>`

### Somebody "corrects" the pkgver

`pkgver` deletes the pre-release hyphen: `1.0.0-alpha.2` becomes `1.0.0alpha.2`. Not a
dot, not an underscore. Confirmed against alpm's `rpmvercmp`: `1.0.0alpha.2` sorts
**below** `1.0.0` while both `1.0.0.alpha.2` and `1.0.0_alpha.2` sort **above** it,
which would make the real 1.0.0 look like a downgrade to every installed user. The
ArchWiki's generic "replace hyphens with underscores" advice has exactly that bug for
pre-release markers. No `epoch` is needed now or at 1.0.0, since `1.0.0alpha.2` already
sorts above `robrix-bin`'s `0.0.1.pre.alpha.4`.

`validate.sh` runs this check with the real `vercmp` in the container:

```sh
docker run --rm archlinux:base-devel bash -c '
  for v in 1.0.0alpha.2 1.0.0.alpha.2 1.0.0_alpha.2; do
    printf "%-16s vs 1.0.0 -> %s\n" "$v" "$(vercmp "$v" 1.0.0)"
  done'
# expect: 1.0.0alpha.2 -> -1 (correct), the other two -> 1 (the downgrade bug)
```
