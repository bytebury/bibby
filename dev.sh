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
  cat >.env <<EOF
PORT=8080

# Display name of the app (page titles, etc.). Defaults to "Bibby" when unset.
APP_NAME=Bibby

# Public origin used for OAuth state target + cookie secure flag.
APP_ORIGIN=http://localhost:8080

DATABASE_URL=postgresql://localhost:5432/postgres

JWT_SECRET=CHANGE_ME_IN_PROD

# Google OAuth — register a single redirect URI in the Google Console
# (the one matching this env's APP_ORIGIN). Preview deploys can share prod's
# callback by listing them in OAUTH_ALLOWED_TARGETS on prod.
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GOOGLE_CALLBACK_URL=http://localhost:8080/auth/google/callback

# Comma-separated allowlist for OAuth state targets when this env acts as the
# registered proxy for preview environments. Wildcard \`*\` allowed once per rule.
OAUTH_ALLOWED_TARGETS=localhost:8080

# Optional \`geodude\` microservice base URL for IP → country/region lookup at
# sign-in. Leave unset locally (or for IPs the service can't resolve) and
# users register against the auto-created "Unknown" country.
GEODUDE_URL=http://localhost:8081

# Include your stripe secrets
STRIPE_SECRET_KEY=
STRIPE_WEBHOOK_SECRET=
EOF
  echo "✅ .env generated."
else
  echo "✅ .env file found."
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
