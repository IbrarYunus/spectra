#!/usr/bin/env bash
# Cut a new spectra release: tag, push, publish on GitHub, and bump the
# Homebrew formula in IbrarYunus/homebrew-spectra.
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
TARBALL_URL="https://github.com/${REPO}/archive/refs/tags/${TAG}.tar.gz"

say()  { printf "\033[1;36m==>\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m!!\033[0m %s\n" "$*"; }
run()  { if [[ "$DRY_RUN" == "--dry-run" ]]; then printf "  \033[2m(dry)\033[0m %s\n" "$*"; else eval "$@"; fi; }

# ── sanity checks ────────────────────────────────────────────────────────
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$ ]]; then
  echo "error: version must be semver (e.g. 0.2.0), got: $VERSION" >&2
  exit 1
fi

for tool in git gh curl shasum sed; do
  command -v "$tool" >/dev/null || { echo "error: missing $tool" >&2; exit 1; }
done

# Must be in the spectra repo root
if [[ ! -f Cargo.toml ]] || ! grep -q '^name = "spectra"' Cargo.toml; then
  echo "error: run this from the spectra repo root" >&2
  exit 1
fi

# Clean working tree
if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree not clean — commit or stash first" >&2
  exit 1
fi

# Tag must not already exist
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists locally" >&2
  exit 1
fi
if git ls-remote --exit-code --tags origin "${TAG}" >/dev/null 2>&1; then
  echo "error: tag ${TAG} already exists on origin" >&2
  exit 1
fi

# Cargo.toml version should match (warn, don't block — user may have bumped separately)
CARGO_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')
if [[ "$CARGO_VERSION" != "$VERSION" ]]; then
  warn "Cargo.toml version is ${CARGO_VERSION}, releasing ${VERSION}. Update Cargo.toml if this is intentional."
fi

say "Releasing ${TAG}"

# ── tag + push ──────────────────────────────────────────────────────────
run "git tag -a \"${TAG}\" -m \"spectra ${VERSION}\""
run "git push origin \"${TAG}\""

# ── GitHub release ──────────────────────────────────────────────────────
say "Creating GitHub release"
run "gh release create \"${TAG}\" --title \"spectra ${VERSION}\" --generate-notes"

# ── compute sha256 of the tarball ───────────────────────────────────────
say "Computing sha256 of ${TARBALL_URL}"
if [[ "$DRY_RUN" == "--dry-run" ]]; then
  SHA="<would-download-and-hash>"
else
  # GitHub takes a moment to materialise archive tarballs after tag push.
  for attempt in 1 2 3 4 5; do
    if SHA=$(curl -fsSL "${TARBALL_URL}" | shasum -a 256 | awk '{print $1}') && [[ -n "$SHA" ]]; then
      break
    fi
    warn "tarball not ready yet (attempt ${attempt}), retrying in 3s..."
    sleep 3
  done
  [[ -n "${SHA:-}" ]] || { echo "error: failed to download tarball" >&2; exit 1; }
fi
echo "    sha256 = ${SHA}"

# ── bump formula in the tap repo ────────────────────────────────────────
say "Bumping formula in ${TAP}"
TAP_DIR=$(mktemp -d)
trap 'rm -rf "${TAP_DIR}"' EXIT

run "gh repo clone \"${TAP}\" \"${TAP_DIR}\" -- --depth 1"

if [[ "$DRY_RUN" != "--dry-run" ]]; then
  FORMULA="${TAP_DIR}/Formula/${FORMULA_NAME}.rb"
  [[ -f "$FORMULA" ]] || { echo "error: formula not found at $FORMULA" >&2; exit 1; }

  # Replace url and sha256 lines (portable sed: use a backup then delete it)
  sed -i.bak -E \
    -e "s|^(  url \")[^\"]+(\")|\\1${TARBALL_URL}\\2|" \
    -e "s|^(  sha256 \")[^\"]+(\")|\\1${SHA}\\2|" \
    "$FORMULA"
  rm "${FORMULA}.bak"

  # Show diff before committing
  (cd "$TAP_DIR" && git --no-pager diff -- "Formula/${FORMULA_NAME}.rb")
fi

(
  cd "$TAP_DIR"
  run "git add \"Formula/${FORMULA_NAME}.rb\""
  run "git commit -m \"${FORMULA_NAME} ${VERSION}\""
  run "git push"
)

say "Done."
echo
echo "  Release:  https://github.com/${REPO}/releases/tag/${TAG}"
echo "  Install:  brew upgrade ${FORMULA_NAME}  (or re-tap if new)"
