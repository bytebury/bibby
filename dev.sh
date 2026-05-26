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
# public/styles.css automatically.
echo "🎨 Starting Tailwind watcher..."
npx tailwindcss \
  -i ./public/styles/tailwind.css \
  -o ./public/styles.css \
  --watch &

# Start the dev server
echo "🦀 Starting Rust dev server..."
export GIT_HASH=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
cargo watch -x 'run --bin bibby'
