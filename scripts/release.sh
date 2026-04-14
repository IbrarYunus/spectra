#!/usr/bin/env bash
# Cut a new spectra release: tag, push, wait for CI to publish prebuilt
# arm64/x86_64 bottles to the GitHub release, then bump the Homebrew formula
# in IbrarYunus/homebrew-spectra to point at the new archives.
#
# Usage:  scripts/release.sh <version>           # e.g. scripts/release.sh 0.2.0
#         scripts/release.sh <version> --dry-run # show what would happen

set -euo pipefail

if [[ "${1:-}" == "" || "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '2,7p' "$0"
  exit 0
fi

VERSION="$1"
DRY_RUN=${2:-}
TAG="v${VERSION}"
REPO="IbrarYunus/spectra"
TAP="IbrarYunus/homebrew-spectra"
FORMULA_NAME="spectra-vis"
ARM_ARCHIVE="spectra-${VERSION}-arm64-apple-darwin.tar.gz"
INTEL_ARCHIVE="spectra-${VERSION}-x86_64-apple-darwin.tar.gz"
RELEASES_BASE="https://github.com/${REPO}/releases/download/${TAG}"

say()  { printf "\033[1;36m==>\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m!!\033[0m %s\n" "$*"; }
run()  { if [[ "$DRY_RUN" == "--dry-run" ]]; then printf "  \033[2m(dry)\033[0m %s\n" "$*"; else eval "$@"; fi; }

# ── sanity checks ────────────────────────────────────────────────────────
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$ ]]; then
  echo "error: version must be semver (e.g. 0.2.0), got: $VERSION" >&2
  exit 1
fi

for tool in git gh curl shasum awk; do
  command -v "$tool" >/dev/null || { echo "error: missing $tool" >&2; exit 1; }
done

if [[ ! -f Cargo.toml ]] || ! grep -q '^name = "spectra"' Cargo.toml; then
  echo "error: run this from the spectra repo root" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree not clean — commit or stash first" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally" >&2
  exit 1
fi
if git ls-remote --exit-code --tags origin "${TAG}" >/dev/null 2>&1; then
  echo "error: tag ${TAG} already exists on origin" >&2
  exit 1
fi

CARGO_VERSION=$(grep -m1 '^version = ' Cargo.toml | awk -F'"' '{print $2}')
if [[ "$CARGO_VERSION" != "$VERSION" ]]; then
  warn "Cargo.toml version is ${CARGO_VERSION}, releasing ${VERSION}. Update Cargo.toml if this is intentional."
fi

say "Releasing ${TAG}"

# ── tag + push (triggers the release workflow) ──────────────────────────
run "git tag -a \"${TAG}\" -m \"spectra ${VERSION}\""
run "git push origin \"${TAG}\""

if [[ "$DRY_RUN" == "--dry-run" ]]; then
  say "Dry-run: skipping CI wait and formula bump"
  exit 0
fi

# ── wait for the workflow to publish both archives ──────────────────────
say "Waiting for CI to publish release artifacts (up to ~20 min)"
printf "  "
MAX_ATTEMPTS=80   # 80 × 15s = 20min
for attempt in $(seq 1 $MAX_ATTEMPTS); do
  ASSETS=$(gh release view "$TAG" --repo "$REPO" --json assets -q '.assets[].name' 2>/dev/null || true)
  if echo "$ASSETS" | grep -qx "$ARM_ARCHIVE" && \
     echo "$ASSETS" | grep -qx "$INTEL_ARCHIVE" && \
     echo "$ASSETS" | grep -qx "${ARM_ARCHIVE}.sha256" && \
     echo "$ASSETS" | grep -qx "${INTEL_ARCHIVE}.sha256"; then
    printf "\n"
    say "Release assets are live"
    break
  fi
  printf "."
  if [[ $attempt -eq $MAX_ATTEMPTS ]]; then
    printf "\n"
    echo "error: timed out waiting for release assets. Check https://github.com/${REPO}/actions" >&2
    exit 1
  fi
  sleep 15
done

# ── fetch the published checksums ───────────────────────────────────────
say "Fetching sha256 checksums"
ARM_SHA=$(curl -fsSL "${RELEASES_BASE}/${ARM_ARCHIVE}.sha256" | awk '{print $1}')
INTEL_SHA=$(curl -fsSL "${RELEASES_BASE}/${INTEL_ARCHIVE}.sha256" | awk '{print $1}')
echo "    arm64:  $ARM_SHA"
echo "    x86_64: $INTEL_SHA"
if [[ -z "$ARM_SHA" || -z "$INTEL_SHA" ]]; then
  echo "error: could not fetch one or both sha256 files" >&2
  exit 1
fi

# ── regenerate the formula in the tap ───────────────────────────────────
say "Bumping formula in ${TAP}"
TAP_DIR=$(mktemp -d)
trap 'rm -rf "$TAP_DIR"' EXIT

gh repo clone "$TAP" "$TAP_DIR" -- --depth 1 >/dev/null

cat > "$TAP_DIR/Formula/${FORMULA_NAME}.rb" <<EOF
class SpectraVis < Formula
  desc "Fast terminal music visualiser with ten styles and ScreenCaptureKit audio"
  homepage "https://github.com/IbrarYunus/spectra"
  version "${VERSION}"
  license "MIT"

  depends_on :macos

  on_macos do
    on_arm do
      url "${RELEASES_BASE}/${ARM_ARCHIVE}"
      sha256 "${ARM_SHA}"
    end
    on_intel do
      url "${RELEASES_BASE}/${INTEL_ARCHIVE}"
      sha256 "${INTEL_SHA}"
    end
  end

  def install
    bin.install "spectra"
    lib.install "libspectra_sc.dylib"
  end

  test do
    assert_match "spectra", shell_output("#{bin}/spectra --version")
  end
end
EOF

(cd "$TAP_DIR" && git --no-pager diff -- "Formula/${FORMULA_NAME}.rb")
(cd "$TAP_DIR" && git add "Formula/${FORMULA_NAME}.rb" && git commit -m "${FORMULA_NAME} ${VERSION}" && git push)

say "Done."
echo
echo "  Release:  https://github.com/${REPO}/releases/tag/${TAG}"
echo "  Install:  brew upgrade ${FORMULA_NAME}    # existing users"
echo "            brew tap ${TAP%/*}/spectra && brew install ${FORMULA_NAME}   # new users"
