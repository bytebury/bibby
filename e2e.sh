#!/usr/bin/env bash
#
# Run the Playwright suite against a freshly-built debug binary.
#
# Spins up a dedicated test instance on port 8090, then tears it down on exit.
# Extra args are forwarded to `playwright test`, e.g.:
#
#   ./e2e.sh                       # run the whole suite
#   ./e2e.sh landing.spec.ts       # run a single spec
#   ./e2e.sh --ui                  # open the Playwright UI
#   ./e2e.sh --headed --debug      # debug a flaky test
#
set -euo pipefail

cd "$(dirname "$0")"

PORT="${E2E_PORT:-8090}"

APP_LOG="$(mktemp -t bibby-e2e-XXXXXX.log)"
APP_PID=""

cleanup() {
  if [[ -n "${APP_PID}" ]] && kill -0 "${APP_PID}" 2>/dev/null; then
    kill "${APP_PID}" 2>/dev/null || true
    wait "${APP_PID}" 2>/dev/null || true
  fi
  if [[ -n "${KEEP_LOG:-}" ]]; then
    echo "🪵 app log preserved at ${APP_LOG}"
  else
    rm -f "${APP_LOG}"
  fi
}
trap cleanup EXIT

command -v cargo >/dev/null || { echo "❌ cargo not on PATH"; exit 1; }
command -v npx >/dev/null || { echo "❌ npx not on PATH (install Node)"; exit 1; }

# Build the app (debug profile — same as CI)
echo "🦀 building (debug)..."
cargo build --bin bibby --quiet

# Build the Tailwind stylesheet so the e2e binary serves the same CSS users do.
echo "🎨 building tailwind..."
npx tailwindcss -i ./public/styles/tailwind.css -o ./public/styles.css --minify

# Node deps & browsers (only the first run pays for these)
if [[ ! -d node_modules ]]; then
  echo "📦 installing npm deps..."
  npm install
fi

# Idempotent: `playwright install` no-ops when the matching browser is
# already cached, but downloads the right build when the cli was upgraded.
echo "🎭 ensuring Chromium is installed..."
npx playwright install chromium >/dev/null

# Boot the app
echo "🚀 starting app on http://127.0.0.1:${PORT}"
PORT="${PORT}" \
APP_NAME="Bibby" \
GIT_HASH="$(git rev-parse HEAD 2>/dev/null || echo unknown)" \
./target/debug/bibby >"${APP_LOG}" 2>&1 &
APP_PID=$!

echo "⏳ waiting for the app..."
if ! npx --yes wait-on -t 60000 "http://127.0.0.1:${PORT}/" 2>/dev/null; then
  echo "❌ app did not become ready — last 50 log lines:"
  tail -n 50 "${APP_LOG}"
  KEEP_LOG=1
  exit 1
fi

# Run Playwright
echo "🎭 running Playwright..."
set +e
PORT="${PORT}" \
BASE_URL="http://127.0.0.1:${PORT}" \
npx playwright test "$@"
status=$?
set -e

if [[ "${status}" -ne 0 ]]; then
  echo "❌ Playwright failed — last 50 app log lines:"
  tail -n 50 "${APP_LOG}"
  KEEP_LOG=1
fi
exit "${status}"
