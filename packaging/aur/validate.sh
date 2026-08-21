#!/usr/bin/env bash
#
# End-to-end check of the Robrix AUR package, on any Linux box with docker.
#
#   ./packaging/aur/validate.sh --tag v1.0.0-alpha.2
#   ./packaging/aur/validate.sh --tag v1.0.0-alpha.3 --with-binfmt
#
# It renders the PKGBUILD once, then per arch builds it with makepkg in an Arch
# container, lints it with namcap, installs the result with pacman -U so the real
# dependency resolution runs, and runs ldd on the installed binary. x86_64 is
# native on an x86_64 host; aarch64 needs qemu/binfmt there (--with-binfmt), and
# is skipped with a clear message when the release ships no aarch64 payload, which
# is the case for every release up to and including v1.0.0-alpha.2.
#
# Everything lands in one mktemp dir. The only thing touched outside it is the
# binfmt registration, and only with --with-binfmt.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RENDER="$SCRIPT_DIR/render-pkgbuild.sh"

tag=''
repo='project-robius/robrix'
arches='x86_64 aarch64'
with_binfmt=0
keep=0

pass=0; fail=0; skip=0
results=''

say()  { printf '%s\n' "$*"; }
note() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()   { pass=$(( pass + 1 )); results="${results}PASS  $*"$'\n'; printf '\033[32mPASS\033[0m  %s\n' "$*"; }
bad()  { fail=$(( fail + 1 )); results="${results}FAIL  $*"$'\n'; printf '\033[31mFAIL\033[0m  %s\n' "$*"; }
meh()  { skip=$(( skip + 1 )); results="${results}SKIP  $*"$'\n'; printf '\033[33mSKIP\033[0m  %s\n' "$*"; }
die()  { printf 'Error: %s\n' "$*" >&2; exit 1; }

while (( $# )); do
    case "$1" in
        --tag)         [[ $# -ge 2 ]] || die "--tag needs a value"; tag="$2"; shift 2 ;;
        --repo)        [[ $# -ge 2 ]] || die "--repo needs a value"; repo="$2"; shift 2 ;;
        --arch)        [[ $# -ge 2 ]] || die "--arch needs a value"; arches="$2"; shift 2 ;;
        --with-binfmt) with_binfmt=1; shift ;;
        --keep)        keep=1; shift ;;
        -h|--help)     sed -n '2,18p' "$0"; exit 0 ;;
        *)             die "unknown argument: $1" ;;
    esac
done

[[ -n "$tag" ]] || die "pass --tag vX.Y.Z, e.g. --tag v1.0.0-alpha.2"
[[ -x "$RENDER" ]] || die "$RENDER is missing or not executable"
command -v docker >/dev/null 2>&1 || die "docker is required"
docker info >/dev/null 2>&1 || die "the docker daemon is not reachable from here"

image_for() {
    case "$1" in
        ## Official Arch is x86_64 only, so aarch64 uses Arch Linux ARM, which is
        ## the distro that package is for anyway.
        x86_64)  printf 'archlinux:base-devel' ;;
        aarch64) printf 'menci/archlinuxarm:base-devel' ;;
        *)       return 1 ;;
    esac
}
docker_platform_for() {
    case "$1" in
        x86_64)  printf 'linux/amd64' ;;
        aarch64) printf 'linux/arm64' ;;
    esac
}

work="$(mktemp -d)"
cleanup() {
    ## makepkg runs as an unprivileged uid inside the container and leaves src/ and
    ## pkg/ owned by it, so hand them back before trying to delete anything.
    docker run --rm -v "$work:/w" "$(image_for x86_64)" \
        chown -R "$(id -u):$(id -g)" /w >/dev/null 2>&1 || true
    if (( keep )); then
        printf '\nLeft everything in %s\n' "$work"
    else
        rm -rf "$work"
    fi
}
trap cleanup EXIT

host_arch="$(uname -m)"
[[ "$host_arch" == 'arm64' ]] && host_arch='aarch64'
pkgname="$("$RENDER" --print-pkgname)"
say "Working in $work"
say "Host $host_arch, package $pkgname, tag $tag"

if (( with_binfmt )); then
    note "Registering qemu binfmt handlers"
    say "This needs --privileged because it writes system-wide binfmt_misc entries."
    say "It replaces any existing qemu-user-static handlers here, until reboot."
    say "What is registered now: $(ls /proc/sys/fs/binfmt_misc/ 2>/dev/null | tr '\n' ' ')"
    docker run --privileged --rm tonistiigi/binfmt --install arm64,amd64
fi

# Rendered once. --mode aur emits every arch the release actually has, so the
# per-arch loop below just picks out the payload it needs.
note "Rendering from $tag"
render_dir="$work/render"
mkdir -p "$render_dir"
if ! "$RENDER" --mode aur --tag "$tag" --repo "$repo" --out "$render_dir" 2>&1 | sed 's/^/    /'; then
    bad "rendering failed, nothing else can run"
    printf '\n%s\nRESULT: FAIL\n' "$results"
    exit 1
fi
ok "rendered $render_dir/PKGBUILD"
grep -E '^(pkgname|pkgver|pkgrel|arch|license)=' "$render_dir/PKGBUILD" | sed 's/^/    /'

for arch in $arches; do
    note "$arch"

    if ! image="$(image_for "$arch")"; then
        bad "$arch: not an arch this script knows how to build"
        continue
    fi
    if ! grep -q "^source_${arch}=" "$render_dir/PKGBUILD"; then
        meh "$arch: release $tag ships no ${arch} payload, so the PKGBUILD does not declare it"
        continue
    fi

    platform=()
    if [[ "$arch" != "$host_arch" ]]; then
        if [[ ! -e "/proc/sys/fs/binfmt_misc/qemu-${arch}" ]]; then
            say "    $arch is foreign to this $host_arch host and qemu-$arch is not registered."
            say "    Re-run with --with-binfmt, or run this on an $arch machine."
            meh "$arch: emulation not available"
            continue
        fi
        platform=(--platform "$(docker_platform_for "$arch")")
        say "    Running $image emulated. Expect it to be slow."
    fi

    out="$work/$arch"
    mkdir -p "$out"
    cp "$render_dir/PKGBUILD" "$out/"
    ## makepkg only wants the payload for its own CARCH, so copy just that one.
    cp "$render_dir"/robrix_*_"${arch}".tar.gz "$out/"

    cat > "$out/run.sh" <<'INNER'
#!/bin/bash
set -euo pipefail
cd /work

echo "--- container ---"
( source /etc/makepkg.conf; echo "CARCH=$CARCH" )
pacman-key --init >/dev/null 2>&1
pacman-key --populate >/dev/null 2>&1
## One asset naming scheme across both images; ALARM has differed from Arch here.
printf '\nPKGEXT=.pkg.tar.zst\n' >> /etc/makepkg.conf
pacman -Syu --noconfirm --needed namcap >/dev/null

echo "--- vercmp: 1.0.0alpha.2 must sort BELOW 1.0.0, the other two must not ---"
for v in 1.0.0alpha.2 1.0.0.alpha.2 1.0.0_alpha.2; do
    printf '    %-14s vs 1.0.0 -> %s\n' "$v" "$(vercmp "$v" 1.0.0)"
done
[[ "$(vercmp 1.0.0alpha.2 1.0.0)" -lt 0 ]] || echo "VERCMP-WRONG"

useradd --create-home builder
chown -R builder /work

echo "--- .SRCINFO ---"
sudo -u builder makepkg --printsrcinfo > .SRCINFO
grep -E 'pkgbase|pkgver|pkgrel|arch =|source_|sha256sums_' .SRCINFO | sed 's/^/    /'

echo "--- namcap PKGBUILD ---"
namcap PKGBUILD | tee namcap-pkgbuild.txt || true

echo "--- makepkg ---"
sudo -u builder env PACKAGER="Robrix validate.sh <contact@robius.rs>" \
    makepkg --force --noconfirm --nodeps

pkg="$(ls ./*.pkg.tar.zst)"
pkgname="$(sed -n 's/^pkgname=//p' PKGBUILD)"

echo "--- namcap $pkg ---"
namcap "$pkg" | tee namcap-pkg.txt || true
if grep -q ' E: ' namcap-pkg.txt namcap-pkgbuild.txt; then
    echo "NAMCAP-ERRORS-PRESENT"
fi

echo "--- pacman -U, which resolves the depends array for real ---"
pacman -U --noconfirm "$pkg"
pacman -Qi "$pkgname" | sed -n '1,10p' | sed 's/^/    /'

echo "--- installed files ---"
for f in /usr/bin/robrix /usr/share/applications/robrix.desktop \
         "/usr/share/licenses/${pkgname}/copyright" \
         "/usr/share/licenses/${pkgname}/THIRD-PARTY-NOTICES.html" \
         /usr/lib/robrix/robrix/resources; do
    if [[ -e "$f" ]]; then echo "    ok  $f"; else echo "    MISSING $f"; echo "FILES-MISSING"; fi
done

echo "--- ldd /usr/bin/robrix ---"
ldd /usr/bin/robrix | sed 's/^/    /'
if ldd /usr/bin/robrix | grep -q 'not found'; then
    echo "LDD-MISSING-LIBS"
fi

echo "--- the dlopened and spawned deps namcap cannot see ---"
for so in libEGL.so.1 libdbus-1.so.3; do
    if ldconfig -p | grep -q "$so"; then echo "    ok  $so"; else echo "    MISSING $so"; echo "DLOPEN-MISSING"; fi
done
if command -v xdg-open >/dev/null; then echo "    ok  xdg-open"; else echo "    MISSING xdg-open"; echo "DLOPEN-MISSING"; fi
if [[ -e /etc/ssl/certs/ca-certificates.crt ]]; then echo "    ok  ca-certificates"; else echo "    MISSING ca-certificates"; echo "DLOPEN-MISSING"; fi

echo "CONTAINER-OK"
INNER
    chmod +x "$out/run.sh"

    log="$out/container.log"
    if ! docker run --rm "${platform[@]}" -v "$out:/work" -w /work "$image" \
            bash /work/run.sh > "$log" 2>&1; then
        sed 's/^/    /' "$log" | tail -40
        bad "$arch: the container run failed, full log at $log"
        continue
    fi
    sed 's/^/    /' "$log"

    problem=''
    saw() { grep -q "$1" "$log"; }
    saw 'CONTAINER-OK'          || problem='the container did not finish'
    if saw 'VERCMP-WRONG';          then problem='vercmp does not order the pkgver as documented'; fi
    if saw 'LDD-MISSING-LIBS';      then problem='ldd reports missing libraries'; fi
    if saw 'FILES-MISSING';         then problem='an expected installed file is absent'; fi
    if saw 'DLOPEN-MISSING';        then problem='a dlopened or spawned dependency is not installed'; fi
    if saw 'NAMCAP-ERRORS-PRESENT'; then problem='namcap reported an E:'; fi
    if [[ -n "$problem" ]]; then
        bad "$arch: $problem (see $log)"
    else
        ok "$arch: rendered, built, linted, installed, ldd clean"
    fi
done

note "Summary"
printf '%s' "$results"
printf '\n%d passed, %d failed, %d skipped\n' "$pass" "$fail" "$skip"
if (( fail )); then say "RESULT: FAIL"; exit 1; fi
if (( pass < 2 )); then
    say "RESULT: FAIL (only the render was validated, no package was actually built)"
    exit 1
fi
say "RESULT: PASS"
