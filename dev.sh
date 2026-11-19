#!/bin/bash
set -e

trap "kill 0" EXIT

# Install cargo-watch if not present
if ! command -v cargo-watch &> /dev/null
then
  echo "cargo watch not found. Installing..."
  cargo install cargo-watch
fi

# Check to see if the user set up an environment file
if [ ! -f .env ]; then
  echo "🤖 .env not found. Generating..."
  APP_PORT="${PORT:-8080}"
  JWT_SECRET_VALUE="$(openssl rand -hex 32 2>/dev/null || od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
  # You become the first admin; add others to the list later.
  ADMIN_EMAILS_VALUE="$(git config user.email 2>/dev/null || true)"
  cat >.env <<EOF
PORT=${APP_PORT}

# Display name of the app (page titles, etc.). Defaults to "Bibby" when unset.
APP_NAME=Bibby

# Public origin used for OAuth state target + cookie secure flag.
APP_ORIGIN=http://localhost:${APP_PORT}

# Public origin used for canonical URLs, Open Graph URLs, robots.txt, and sitemap.xml.
PUBLIC_SITE_URL=http://localhost:${APP_PORT}

# SEO defaults. Individual pages can override these in their SharedContext metadata.
SEO_DEFAULT_TITLE=Bibby
SEO_DEFAULT_DESCRIPTION="A full-stack web application with sign-in, billing, blogs, and more."
SEO_DEFAULT_IMAGE=/assets/images/app-icon.svg
SEO_TWITTER_HANDLE=
SEO_ROBOTS=index,follow

# Installable web-app defaults.
WEB_APP_NAME=Bibby
WEB_APP_SHORT_NAME=Bibby
WEB_APP_THEME_COLOR="#111827"
WEB_APP_BACKGROUND_COLOR="#f9fafb"
WEB_APP_DISPLAY=standalone
WEB_APP_SERVICE_WORKER=false

DATABASE_URL=postgresql://localhost:5432/postgres
# Optional pool size per server (default 25).
# DATABASE_MAX_CONNECTIONS=25

# Signs sessions and OAuth state and keys the sign-in code hashes. At least 32
# characters; generate a new one for each environment.
JWT_SECRET=${JWT_SECRET_VALUE}

# Required. Comma-separated addresses that become admins once they sign in
# (verified Google sign-in or an emailed code). Whitespace is trimmed.
ADMIN_EMAILS=${ADMIN_EMAILS_VALUE}

# How many reverse proxies sit in front of the app (Railway: 1). The client IP
# is read that many entries from the right of X-Forwarded-For. 0 ignores it.
TRUSTED_PROXY_HOPS=0

# Google OAuth, optional: leave the id and secret empty and the "Continue
# with Google" button is hidden. Register a single redirect URI in the Google Console
# (the one matching this env's APP_ORIGIN). Preview deploys can share prod's
# callback by listing them in OAUTH_ALLOWED_TARGETS on prod.
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GOOGLE_CALLBACK_URL=http://localhost:${APP_PORT}/auth/google/callback

# Comma-separated allowlist for OAuth state targets when this env acts as the
# registered proxy for preview environments. Wildcard \`*\` allowed once per rule.
OAUTH_ALLOWED_TARGETS=localhost:${APP_PORT}

# Optional \`geodude\` microservice base URL for IP → country/region lookup at
# sign-in. Leave it empty locally (or for IPs the service can't resolve) and
# users register against the auto-created "Unknown" country.
GEODUDE_URL=

# \`mailman\` microservice that sends email (sign-in and verification codes).
# Leave MAILMAN_URL empty locally and codes fail to send (the error is logged).
# MAILMAN_API_TOKEN is optional: set it only if your mailman requires one.
MAILMAN_URL=
MAILMAN_API_TOKEN=

# Shared secret for POST /_internal/cron/tick, which runs the daily jobs (plan
# expirations, sign-in pruning). Point a daily cron at it with the header
# \`X-Cron-Secret: <this value>\`.
CRON_SECRET=

# Stripe, optional: leave STRIPE_SECRET_KEY empty and billing is switched off
# (no Upgrade or Billing links, and its routes answer 404). The price ids are
# the Stripe prices for each plan on /upgrade.
STRIPE_SECRET_KEY=
STRIPE_WEBHOOK_SECRET=
STRIPE_MONTHLY_PRO=
STRIPE_ANNUAL_PRO=
EOF
  echo "✅ .env generated."
else
  echo "✅ .env file found."
fi

# Export .env values for sqlx and the Rust process.
set -a
source .env
set +a

if [ -z "${DATABASE_URL:-}" ]; then
  echo "❌ DATABASE_URL is not set in .env"
  exit 1
fi

# The app refuses to start without these, so say so before building it.
if [ -z "${ADMIN_EMAILS// /}" ]; then
  echo "❌ ADMIN_EMAILS is not set in .env. Set it to your email address (comma-separate several)."
  exit 1
fi
if [ "${#JWT_SECRET}" -lt 32 ] || [ "${JWT_SECRET}" = "CHANGE_ME_IN_PROD" ]; then
  echo "❌ JWT_SECRET in .env must be at least 32 characters. Generate one with: openssl rand -hex 32"
  exit 1
fi

# Create the database if needed and run the migrations.
if ! sqlx database setup; then
  echo "❌ database setup failed for DATABASE_URL=${DATABASE_URL}"
  echo "   Make sure Postgres is running and reachable, then run ./dev.sh again."
  exit 1
fi
echo "✅ database setup completed."

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git config core.hooksPath .githooks
  echo "✅ Git hooks enabled."
fi

# Install npm deps if missing (Tailwind + Playwright live here)
if [ ! -d node_modules ]; then
  echo "📦 installing npm deps..."
  npm install
fi

# Start the tailwind watcher in the background so HTML/CSS edits regenerate
# public/styles.css automatically. `--watch=always` keeps the watcher alive
# when stdin closes — without it, IDE run consoles (RustRover) trigger a single
# build at startup and then the watcher exits, leaving styles.css stale.
echo "🎨 Starting Tailwind watcher..."
npx tailwindcss \
  -i ./public/styles/tailwind.css \
  -o ./public/styles.css \
  --watch=always &

# Start the dev server in the foreground; the EXIT trap kills the tailwind bg job.
echo "🦀 Starting Rust dev server..."
export GIT_HASH=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
cargo watch -x 'run --bin bibby'
