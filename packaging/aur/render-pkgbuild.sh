#!/usr/bin/env bash
#
# Renders packaging/arch/PKGBUILD.in into a real PKGBUILD. One template, two ways:
#
#   --mode local   the release build. The payload tarball already sits next to the
#                  PKGBUILD, so sources are bare filenames with SKIP checksums.
#   --mode aur     the AUR. Sources are release-asset URLs with real sha256s, and we
#                  ask the release which arch payloads it actually has, so a new arch
#                  turns itself on the first release that ships one.
#
#   render-pkgbuild.sh --mode local --srcver 1.0.0-alpha.2 --arch x86_64 --out DIR
#   render-pkgbuild.sh --mode aur   --tag v1.0.0-alpha.2 [--repo owner/name] [--pkgrel N] --out DIR
#   render-pkgbuild.sh --print-pkgname
#   render-pkgbuild.sh --print-pkgver --tag v1.0.0-alpha.2
#
# aur mode leaves each downloaded payload in --out under the name the PKGBUILD
# expects, so a following `makepkg` verifies the checksums without re-downloading.
# Progress goes to stderr, so stdout stays usable for the --print-* forms.
#
# Plain bash 3.2, so it runs on stock macOS as well as on Arch and the runners.

set -euo pipefail

## THE ONE-LINE NAME SWITCH. Set this to robrix-bin to publish under the -bin
## suffix instead; provides, conflicts and the license dir all follow it.
PKGNAME='robrix'

## Upstream's own name, which is what cargo-packager names the payload after and
## what the payload's own /usr/lib and /usr/share/licenses paths use.
APPNAME='robrix'
DEFAULT_REPO='project-robius/robrix'
## Arches we know how to publish, in the order they get emitted.
CANDIDATE_ARCHES='x86_64 aarch64'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEMPLATE="$SCRIPT_DIR/../arch/PKGBUILD.in"
NL=$'\n'

log()  { printf '%s\n' "$*" >&2; }
die()  { printf 'Error: %s\n' "$*" >&2; exit 1; }
need() { [[ $2 -ge 2 ]] || die "$1 needs a value"; }

mode='aur' srcver='' tag='' arch='' out='' pkgrel=1 print=''
repo="$DEFAULT_REPO"

while (( $# )); do
    case "$1" in
        --mode)          need "$1" $#; mode="$2"; shift 2 ;;
        --srcver)        need "$1" $#; srcver="$2"; shift 2 ;;
        --tag)           need "$1" $#; tag="$2"; shift 2 ;;
        --arch)          need "$1" $#; arch="$2"; shift 2 ;;
        --repo)          need "$1" $#; repo="$2"; shift 2 ;;
        --pkgrel)        need "$1" $#; pkgrel="$2"; shift 2 ;;
        --out)           need "$1" $#; out="$2"; shift 2 ;;
        --print-pkgname) print='pkgname'; shift ;;
        --print-pkgver)  print='pkgver'; shift ;;
        -h|--help)       sed -n '2,20p' "$0" >&2; exit 0 ;;
        *)               die "unknown argument: $1" ;;
    esac
done

if [[ "$print" == 'pkgname' ]]; then
    printf '%s\n' "$PKGNAME"
    exit 0
fi

## Drop the first hyphen only, so 1.0.0-alpha.2 becomes 1.0.0alpha.2. The note at
## the top of PKGBUILD.in explains why it isn't a dot or an underscore.
to_pkgver() {
    local v="${1/-/}"
    printf '%s' "${v//-/.}"
}

check_tag() {
    [[ -n "$tag" ]] || die "--tag is required"
    ## The emitted source URLs rebuild the tag as v${_srcver}, so anything else would
    ## hash one URL and publish another.
    [[ "$tag" == v* ]] || die "--tag must start with 'v', got '$tag'"
}

if [[ "$print" == 'pkgver' ]]; then
    check_tag
    to_pkgver "${tag#v}"
    printf '\n'
    exit 0
fi

[[ "$mode" == 'aur' || "$mode" == 'local' ]] || die "--mode must be aur or local, got '$mode'"
[[ -n "$out" ]] || die "--out DIR is required"
[[ -f "$TEMPLATE" ]] || die "template not found at $TEMPLATE"
[[ "$pkgrel" =~ ^[1-9][0-9]*$ ]] || die "--pkgrel must be a positive integer, got '$pkgrel'"
mkdir -p "$out"

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

## One ranged GET, so an arch with no payload costs a 404 instead of 70 MB. curl
## already prints 000 itself when it can't connect, so no fallback echo here.
probe() {
    curl -sSL -r 0-0 -o /dev/null -w '%{http_code}' \
        --retry 5 --retry-delay 3 --connect-timeout 20 --max-time 120 "$1" 2>/dev/null || true
}

## Download to .part first, so an interrupted run can't leave a truncated file that
## we would then happily checksum.
download() {
    curl -fL --no-progress-meter --retry 8 --retry-all-errors --retry-delay 3 \
        --connect-timeout 20 --max-time 1800 -o "${2}.part" "$1"
    mv "${2}.part" "$2"
}

sources=''
found=''

if [[ "$mode" == 'local' ]]; then
    [[ -n "$srcver" || -n "$tag" ]] || die "--mode local needs --srcver (or --tag)"
    [[ -n "$srcver" ]] || { check_tag; srcver="${tag#v}"; }
    ## uname says arm64 on macOS and aarch64 on Linux; makepkg only knows the latter.
    [[ -n "$arch" ]] || arch="$(uname -m)"
    [[ "$arch" == 'arm64' ]] && arch='aarch64'
    case " $CANDIDATE_ARCHES " in
        *" $arch "*) ;;
        *) die "'$arch' is not one of the arches we package: $CANDIDATE_ARCHES" ;;
    esac
    found="$arch"
    printf -v sources '%s\n' \
        '## Copied in next to this PKGBUILD by the release workflow, so nothing here' \
        '## came off the network and there is nothing to verify.' \
        "_srcver=${srcver}" \
        '' \
        "source_${arch}=(\"${APPNAME}_\${_srcver}_${arch}.tar.gz\")" \
        "sha256sums_${arch}=('SKIP')"
    log "==> local: ${arch}, checksums skipped (it's a build artifact sitting right there)"
else
    check_tag
    srcver="${tag#v}"
    baseurl="https://github.com/${repo}/releases/download/${tag}"

    arch_block=''
    for a in $CANDIDATE_ARCHES; do
        asset="${APPNAME}_${srcver}_${a}.tar.gz"
        code="$(probe "${baseurl}/${asset}")"
        case "$code" in
            200|206) ;;
            404) log "  --  ${a}: no ${asset} on ${tag}, leaving that arch out"; continue ;;
            *)   die "probing ${baseurl}/${asset} returned HTTP ${code:-000}" ;;
        esac

        dest="${out}/${asset}"
        if [[ -f "$dest" ]]; then
            log "  ..  ${a}: reusing ${asset} already in ${out}"
        else
            log "  ..  ${a}: fetching ${asset}"
            download "${baseurl}/${asset}" "$dest"
        fi
        sum="$(sha256_of "$dest")"
        log "  ok  ${a}: ${sum}"

        found="${found:+$found }${a}"
        printf -v one '%s\n' \
            '' \
            "source_${a}=(\"\${_baseurl}/${APPNAME}_\${_srcver}_${a}.tar.gz\")" \
            "sha256sums_${a}=('${sum}')"
        arch_block="${arch_block}${one}"
    done

    [[ -n "$found" ]] || die "release ${tag} ships no ${APPNAME}_${srcver}_<arch>.tar.gz payload.
If it is still a draft, publish it first: draft assets 404 for anonymous downloads."

    printf -v sources '%s\n' \
        '## Named by cargo-packager and pinned by `asset_name_template: __FILENAME__`' \
        '## in release.yml, so renaming it there breaks this. The name already carries' \
        '## the version and arch, so it needs no `::` rename to stay unique.' \
        "_srcver=${srcver}" \
        "_baseurl=\"https://github.com/${repo}/releases/download/v\${_srcver}\""
    sources="${sources}${arch_block}"
fi

## printf leaves a trailing newline the template already provides.
sources="${sources%"$NL"}"

arches=''
for a in $found; do arches="${arches:+$arches }'${a}'"; done

pkgver="$(to_pkgver "$srcver")"

## Provide whichever prebuilt name we are not, so either package drops in for the
## other, and conflict with all three since they all own /usr/bin/robrix.
provides='' conflicts=''
for n in "$APPNAME" "${APPNAME}-bin"; do
    [[ "$n" == "$PKGNAME" ]] || provides="${provides:+$provides }\"${n}=\${pkgver}\""
done
for n in "$APPNAME" "${APPNAME}-bin" "${APPNAME}-git"; do
    [[ "$n" == "$PKGNAME" ]] || conflicts="${conflicts:+$conflicts }'${n}'"
done

## The payload names its license dir after the app; namcap wants it named after the
## package. Emitted only when those differ, so the rendered file has no dead branch.
fixup=''
if [[ "$PKGNAME" != "$APPNAME" ]]; then
    printf -v fixup '%s\n' \
        '' \
        "    ## namcap looks for the license files under \$pkgname. Nothing reads this" \
        "    ## path at runtime, unlike /usr/lib/${APPNAME}, so renaming it is safe." \
        "    mv \"\${pkgdir}/usr/share/licenses/${APPNAME}\" \"\${pkgdir}/usr/share/licenses/\${pkgname}\""
fi

## awk -v can't carry a newline portably, so the two multi-line blocks go via files.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf '%s\n' "$sources" > "$tmp/sources"
printf '%s' "$fixup" > "$tmp/fixup"

awk -v sourcesfile="$tmp/sources" -v fixupfile="$tmp/fixup" \
    -v pkgname="$PKGNAME" -v pkgver="$pkgver" -v pkgrel="$pkgrel" \
    -v arches="$arches" -v url="https://github.com/${repo}" \
    -v provides="$provides" -v conflicts="$conflicts" '
    index($0, "@SOURCES@") {
        while ((getline line < sourcesfile) > 0) print line
        close(sourcesfile); next
    }
    index($0, "@LICENSE_DIR_FIXUP@") {
        while ((getline line < fixupfile) > 0) print line
        close(fixupfile); next
    }
    {
        gsub(/@PKGNAME@/, pkgname); gsub(/@PKGVER@/, pkgver); gsub(/@PKGREL@/, pkgrel)
        gsub(/@ARCH@/, arches);     gsub(/@URL@/, url)
        gsub(/@PROVIDES@/, provides); gsub(/@CONFLICTS@/, conflicts)
        print
    }
' "$TEMPLATE" > "$tmp/PKGBUILD"

left="$(grep -o '@[A-Z][A-Z_]*@' "$tmp/PKGBUILD" | sort -u | tr '\n' ' ' || true)"
[[ -z "$left" ]] || die "unsubstituted placeholder(s): ${left}"
bash -n "$tmp/PKGBUILD" || die "the rendered PKGBUILD is not valid shell"

cp "$tmp/PKGBUILD" "${out}/PKGBUILD"
log "==> Wrote ${out}/PKGBUILD  (pkgname=${PKGNAME} pkgver=${pkgver} pkgrel=${pkgrel} arch=${found})"

printf 'pkgname=%s\n' "$PKGNAME"
printf 'pkgver=%s\n'  "$pkgver"
printf 'pkgrel=%s\n'  "$pkgrel"
printf 'arches=%s\n'  "$found"
