#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/docker-library/docs.git"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
DEST_DIR="${PROJECT_ROOT}/assets/images/docker-icons"
COLOR_FILE="${DEST_DIR}/colors.json"

WORK_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

require_command git
require_command find
require_command node

mkdir -p "${DEST_DIR}"

echo "Cloning docker-library/docs..."
git clone --depth 1 "${REPO_URL}" "${WORK_DIR}/docs" >/dev/null

count=0
while IFS= read -r logo_path; do
  library_name="$(basename "$(dirname "${logo_path}")")"
  cp "${logo_path}" "${DEST_DIR}/${library_name}.png"
  count=$((count + 1))
done < <(find "${WORK_DIR}/docs" -mindepth 2 -maxdepth 2 -type f -name 'logo.png' | sort)

if [[ ! -d "${SCRIPT_DIR}/node_modules/colorthief" ]]; then
  echo "error: missing scripts/node_modules/colorthief" >&2
  echo "Run: npm install --prefix scripts" >&2
  exit 1
fi

echo "Extracting dominant icon colors..."
node "${SCRIPT_DIR}/extract-docker-icon-colors.mjs" "${DEST_DIR}" "${COLOR_FILE}"

echo "Fetched ${count} Docker official image icons into ${DEST_DIR}"
echo "Wrote dominant colors to ${COLOR_FILE}"
