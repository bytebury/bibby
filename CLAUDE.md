# CLAUDE.md

Guidance for AI assistants working in this repository. `AGENTS.md` covers the
template-level gotchas (askama macro quirks, modals, tooltips, spinners, the
tailwind watcher) — read both before making changes.

## Core values

Every change is judged on these, in this order:

1. **Simplicity.** Code a person or an AI can read once and understand. Remove
   duplication with a helper or a small macro (see [Macros](#macros)), not a
   framework. No speculative abstractions.
2. **Correctness.** Sound logic, unit tests for the rules, e2e tests for the
   flows, and both suites green before anything is pushed.
3. **Performance.** As fast as reasonably possible: the app (no needless
   queries or per-request work), the tests, and CI.

## Project

**Bibby** — Bytebury's full-stack Rust template. Server-rendered with HTMX; no
JS framework.

- Binary crate: `bibby` (Rust edition 2024)
- Server: Axum 0.8 on a multi-thread `tokio` runtime, listens on `$PORT`
- Views: Askama 0.16 + `askama_web` (renders directly into Axum responses)
- Styles: Tailwind v4 (`public/styles/tailwind.css` → generated `public/styles.css`)
- DB: PostgreSQL via SQLx **0.9** (compile-time-checked queries, migrations
  embedded)
- Auth: Google OAuth or an emailed one-time code → JWT in the `auth_token`
  cookie
  - `/sign-up` creates an account from first name, last name, and any email
    (`users.auth_provider = 'email'`, random farm-animal avatar from
    `domain/avatar.rs`). It starts `verified = false`; entering the emailed
    code verifies it.
  - `/sign-in` emails a 6-digit code (`one_time_passcodes` table,
    HMAC-hashed, 10-minute TTL, 5 attempts, throttled per address). It never
    reveals whether an email has an account: unknown addresses answer the
    same, every failed code gets the same message, and the email is sent in
    the background so timing matches too.
  - A verified Google sign-in converts an email account to `'google'`, taking
    Google's name and photo. An identity Google hasn't verified never gets a
    session, even for an existing verified account.
  - **Admins come from `ADMIN_EMAILS`** (required, comma-separated, trimmed;
    `domain/value_objects/admin_emails.rs`). A listed address becomes an
    admin once it proves ownership (verified Google sign-in or an emailed
    code). Leaving the list doesn't demote anyone.
  - **Unverified or locked users never get a session:** `session_user` treats them
    as signed out. So does a token whose `uid` or `ver` no longer matches the
    account: `User::end_all_sessions` bumps `users.session_version`, which
    happens on a lock, an admin email change, and "Sign out everywhere" in
    settings.
  - The sign-up and sign-in POSTs are rate limited per IP
    (`infra/rate_limit.rs`, in memory, off while the test shim is on), and
    `infra/csrf.rs` refuses state-changing requests from other sites.
  - Startup refuses a missing or short (< 32 chars) `JWT_SECRET`.
- Payments: Stripe via the `shima` crate, **optional**. Without
  `STRIPE_SECRET_KEY`, `AppState::payments_use_cases` is `None`, the billing
  routes answer 404 (`state.payments()?`), and `shared.payments_enabled`
  hides the links. The webhook follows subscription status (see
  `handle_stripe_event.rs`) and never changes an admin's role.
- Google sign-in is optional the same way (`google::is_configured`,
  `shared.google_enabled`).
- Email: the `mailman` microservice (`infra/email.rs`)
- Geolocation: the `geodude` microservice resolves IPs to country / region /
  city on sign-in

## Running locally

```bash
./dev.sh          # creates .env if missing, sets up the DB, runs tailwind + cargo-watch
./e2e.sh          # Playwright suite against a fresh debug build on port 8090
```

Prerequisites: Rust, Node, `sqlx-cli`, and Postgres reachable at
`DATABASE_URL`. The generated `.env` lists every variable with comments:
`PORT`, `APP_NAME`, `APP_ORIGIN`, `PUBLIC_SITE_URL`, `SEO_*`, `WEB_APP_*`,
`DATABASE_URL`, `DATABASE_MAX_CONNECTIONS`, `JWT_SECRET` (generated),
`ADMIN_EMAILS` (defaults to your `git config user.email`),
`TRUSTED_PROXY_HOPS`, `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`,
`GOOGLE_CALLBACK_URL`, `OAUTH_ALLOWED_TARGETS`, `GEODUDE_URL`, `MAILMAN_URL`,
`MAILMAN_API_TOKEN`, `CRON_SECRET`, `STRIPE_SECRET_KEY`,
`STRIPE_WEBHOOK_SECRET`, `STRIPE_MONTHLY_PRO`, `STRIPE_ANNUAL_PRO`.

Only `DATABASE_URL`, `JWT_SECRET` and `ADMIN_EMAILS` are required; the app
runs without Stripe, Google, mailman and geodude. Not in `.env`:
`ENABLE_TEST_AUTH` / `TEST_AUTH_TOKEN` (the e2e shim, below), `GIT_HASH`
(read at compile time and shown on `/admin/monitoring`; the Dockerfile takes
it as a build arg) and Railway's `RAILWAY_GIT_COMMIT_SHA` (its fallback).

## Before committing

`./dev.sh` points git at `.githooks/`. The pre-commit hook runs
`cargo fmt --all` and `cargo sqlx prepare`, and stops the commit if either
changed files. CI (`.github/workflows/rust.yml`) runs `cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, and checks
that `.sqlx/` is current.

**Before every push, run all of it yourself, including both test suites:**
`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo test`, `cargo sqlx prepare --check`, and `./e2e.sh`. Never push red.
CI's database starts empty, so when a change touches what tests read back
(activity, counts, analytics), run `./e2e.sh` against a freshly created
database too: leftover rows from earlier runs can hide a regression.

- Do not hand-edit `.sqlx/`. Run `cargo sqlx prepare` after changing any
  `query!` / `query_as!` / `query_scalar!` and commit the churn with the code.
  CI and the Docker build compile with `SQLX_OFFLINE=true`.
- Run new migrations (`sqlx migrate run`) before `cargo sqlx prepare`.
- **Always** use conventional commits for commit messages and PR titles
  (`feat: …`, `fix: …`, `refactor: …`, `perf: …`, `test: …`, `docs: …`,
  `chore: …`).

## Branching workflow

- Always branch off `main` and open a PR back into `main`. Never commit to
  `main` directly.

## Directory layout

```
src/
  main.rs                 # entry: dotenv, tracing, DB init, router assembly, static files
  lib.rs                  # AppState — SharedDatabase + use-case bundles, exported as SharedState
  prelude.rs              # common imports (Template, WebTemplate, serde, sqlx prelude, Result, AppError, PrimaryKey, Utc, tracing…)
  error.rs                # AppError enum, From impls, IntoResponse mapping, Result alias
  domain/                 # entities; persistence lives on the entity
    mod.rs                # Access<T> and Validate traits
    user.rs, country.rs, region.rs, city.rs, announcement.rs, blog.rs,
    one_time_passcode.rs, sign_in.rs, user_note.rs, user_flag.rs,
    user_activity.rs, admin_analytics.rs, avatar.rs, system_status.rs
    value_objects/        # Email, AdminEmails, OneTimeCode, Markdown, Severity
  use_cases/              # application logic, grouped by aggregate
    user/, payments/      # each exposes a `<Name>UseCases` bundle mounted on AppState
  infra/
    db.rs                 # PgPool init; exports DbPool, SharedDatabase, PrimaryKey (= i32)
    pagination.rs         # Paginate trait (paginate / paginate_where), Paginated<T>
    geolocation.rs        # geodude lookup
    email.rs              # mailman: sign-in codes, plan-expiry emails
    payments.rs           # Stripe client wrapper (optional)
    markdown.rs           # Markdown → sanitized HTML (pulldown-cmark + ammonia)
    monitoring.rs         # in-memory logs and request stats for /admin/monitoring
    rate_limit.rs         # per-IP limit on the sign-up / sign-in POSTs
    csrf.rs               # refuses cross-site state-changing requests
    scheduler.rs          # the daily tick (plan expirations, sign-in pruning)
    seo.rs                # SeoConfig / WebAppConfig / PageMeta
    assets.rs             # content-hash cache busting + Cache-Control for /assets
    auth/                 # Google OAuth, OAuth state, JWT claims
    api/                  # Axum routers, one file per resource; `extract/` for request guards
templates/                # Askama templates; `_layouts/`, `_partials/`, `_macros/`, one folder per resource
public/                   # static assets served under /assets (scripts, styles, images)
migrations/               # sqlx migrations (YYYYMMDDHHMMSS_<name>.sql)
.sqlx/                    # SQLx offline query cache (generated; checked in)
e2e/                      # Playwright specs + fixtures
```

## Architecture conventions

### Clean-ish, but no repositories
The project follows Clean Architecture where it pays off — but **persistence
lives on the domain entity itself, not in a separate repository**. Static
methods like `User::find_by_email(exec, …)` take any SQLx
`Executor<'e, Database = sqlx::Postgres>`, so callers pass either a pool
(`self.db.as_ref()`) or a transaction (`&mut *tx`). This lets use-cases
compose multi-step transactions cleanly.

```rust
pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<Self>
where E: Executor<'e, Database = sqlx::Postgres> { … }
```

**Do not introduce a repository layer.** If you want one, add a method to the
entity instead.

### Use-cases
Application logic lives in `use_cases/<aggregate>/<verb>_use_case.rs`. Each is
a struct holding a `SharedDatabase` (and any infra clients) with a single
`execute` method returning `Result<…>`. They're aggregated into per-aggregate
bundles (`UserUseCases`, `PaymentsUseCases`) and mounted on `AppState` in
`AppState::new` (`src/lib.rs`). Handlers reach them via
`state.user_use_cases.search.execute(…)`.

When an operation touches multiple rows or entities, open a transaction at the
top of the use-case (`self.db.begin().await?`), thread `&mut *tx` through the
domain calls, and `tx.commit().await?` at the end. See
`use_cases/user/send_sign_in_code_use_case.rs` and
`use_cases/payments/handle_stripe_event.rs`.

### Authorization

**CRITICAL: Every handler that reads or mutates a resource MUST verify the
caller is allowed to access it. Missing an access check is a security bug —
any new route, use-case, or handler you add is incomplete until the check is
in place.** Authentication (a `CurrentUser` / `Admin` extractor) only proves
*who* the caller is; it does **not** prove they may touch the specific entity
referenced by the URL or form.

#### The rule
Whenever a handler loads an entity by an id that came from the request (path
param, query string, form field, HTMX header), you must:

1. Load the entity (and, if needed, the aggregate that owns it).
2. Call `user.ensure_can_read(&entity)?` for GETs / read-only work.
3. Call `user.ensure_can_write(&entity)?` for any mutation
   (POST/PUT/PATCH/DELETE or state-changing use-case).

Both return `Err(AppError::Forbidden(…))` → HTTP 403. Never hand-roll
`if user.id != entity.user_id` comparisons — always go through `Access<T>` +
`ensure_can_*` so the policy lives in one place. This applies even behind the
`Admin` extractor: admin handlers in `infra/api/users.rs` still call
`admin.ensure_can_write(&user)?`, so tightening the policy later is a one-line
change.

#### How the system checks access

`domain/mod.rs` defines the trait. **Note bibby's defaults: reads are allowed
and writes are denied unless the impl says otherwise.**

```rust
pub trait Access<T> {
    fn can_read(&self, _entity: &T) -> bool { true }
    fn can_write(&self, _entity: &T) -> bool { false }
}
```

Because `can_read` defaults to `true`, **any entity that isn't public must
override `can_read` explicitly.** `User::ensure_can_read` /
`ensure_can_write` (in `domain/user.rs`) wrap these and return
`AppError::Forbidden` on denial. Current implementation:

```rust
impl Access<User> for User {
    fn can_read(&self, entity: &User) -> bool { self.is_admin() || self.id == entity.id }
    fn can_write(&self, _: &User) -> bool { self.is_admin() }
}
```

#### Check against the owning aggregate, not the leaf
Entities that hang off another one (an admin note, a flag, a sign-in row all
hang off a `User`) do not get their own
`Access` impl. Load the owner and check access on it, then scope the child
query to that owner so an id from the URL can't reach someone else's row:

```rust
// infra/api/users.rs
let user = User::find_by_id(db, id).await?;
admin.ensure_can_write(&user)?;
UserNote::destroy(db, user.id, note_id).await?;   // DELETE … WHERE id = $1 AND user_id = $2
```

Add a new `impl Access<Foo> for User` only when `Foo` itself carries the
ownership column (e.g. a `user_id`); otherwise route the check through the
parent.

#### Where the check belongs
- **Handlers (`src/infra/api/`)** — always, before calling the use-case or
  rendering.
- **Use-cases** — enforce **plan gating** (`user.is_free()` / `is_pro()` /
  `is_admin()`) and cross-aggregate invariants. If a use-case takes the acting
  user, re-check ownership there too — defense in depth is fine.
- **Admin-only surfaces** — use the `Admin(User)` extractor instead of
  `CurrentUser`; it rejects non-admins with 403 before the handler runs.
- Never trust ids or roles from a form body to decide *who* the caller is;
  derive that from the extractor.

#### Checklist when adding a new route
1. Pick the right extractor (`CurrentUser`, `MaybeCurrentUser`, `Admin`).
2. Load the entity referenced by the path/form.
3. Walk up to the owning aggregate if needed.
4. Call `ensure_can_read` (GET) or `ensure_can_write` (mutations).
5. Only then invoke the use-case / render the template.

### Input: parse, don't validate
- Wrap primitives with invariants in value objects
  (`domain/value_objects/`). `Email::parse` trims, lowercases, and
  shape-checks; downstream code never re-checks.
- For forms, either implement `Validate` (`fn validate(&self)`) and use the
  `ValidatedForm<T>` extractor, or convert the raw form into a parsed type
  with `TryFrom` (`SignUpForm` → `SignUp`, `UserForm` → `UserUpdate`).
- Validation failures are `AppError::BadRequest(message)`; the message is
  what the user sees.

### Errors
Return `Result<T>` from handlers/use-cases. `AppError` maps to status codes in
its `IntoResponse` impl: `BadRequest→400`, `NotFound→404`, `Conflict→409`,
`Forbidden→403`, `Unauthorized→401`, `Internal→500`. Internal errors are
logged and always surface as `"Something went wrong"`. `From<sqlx::Error>`
turns unique violations into `Conflict`; for a per-constraint message match on
the constraint name (see `map_email_conflict` in `domain/user.rs`).

HTMX is configured **not** to swap 4xx/5xx responses; `public/scripts/toast.js`
shows the response body in a toast instead. When an error belongs next to a
field (sign-in codes), return 200 with the re-rendered
fragment and an inline message instead — see `infra/api/auth.rs`.

### HTTP handlers (`src/infra/api/`)
- `api::routes()` merges per-resource routers. New resource: add
  `pub mod foo;` in `api/mod.rs` and `.merge(foo::routes())`.
- Templates are structs deriving `Template` + `WebTemplate` with a
  `#[template(path = "…")]` attribute; returning the struct renders it.
- Page templates take a `shared: SharedContext` field:
  `SharedContext::new(&state).with_user(Some(user)).with_canonical_path("/x").with_meta(PageMeta::new().title("X"))`.
  Admin and private pages set `.robots("noindex,nofollow")`; public pages go
  in `sitemap_xml` in `infra/api/core.rs`.
- **Always redirect with `redirect!(url, &headers)` / `refresh!(&headers)`**
  from `api/mod.rs`, never `Redirect::to`. Every handler that may redirect
  takes a `HeaderMap`.
- Timestamps render as `<time data-utc="{{ x.created_at }}"></time>`.
- JSON embedded in a `<script>` goes through `api::script_json` before
  `|safe`. Plain `serde_json` output doesn't escape `</script>`, so a user's
  name in it would run as script.
- **Link static files through `{{ shared.asset("/assets/…") }}`**, never a
  bare `/assets/…` path. It appends `?v=<content hash>` (see
  `infra/assets.rs`), and release builds cache only a current-hash URL
  forever (`immutable`); anything else gets `max-age=3600`. A unit test fails
  if a template links a `.js` or `.css` file without the helper. Dynamic
  images such as flags may stay bare — they just get the short cache.

### Macros

Reach for a macro when it removes real repetition; keep each one small and
documented here and in the README.

| Macro | Where | What it does |
|---|---|---|
| `redirect!(url, &headers)` | `infra/api/mod.rs` | 303, or `HX-Redirect` for HTMX. Use for every redirect. |
| `refresh!(&headers)` | `infra/api/mod.rs` | Reloads the page (`HX-Refresh` for HTMX). |
| `static_page!(handler, "x.html", "/x", meta)` | `infra/api/mod.rs` | Handler for a page that needs only `SharedContext` (homepage, privacy, terms). |
| `select_user!("WHERE …", args…)` | `domain/user.rs` | `query_as!` for one `User` from `user_details`; the column list lives there once. |
| `ui::spinner_button(…)` | `templates/_macros/buttons.html` | HTMX button with the standard attributes. |
| `pagination::pager(page, base, target)` | `templates/_macros/pagination.html` | Previous / Next links for a `Paginated<T>`; `target` swaps over HTMX. |

### Request extractors (`src/infra/api/extract/`)
| Extractor | Meaning |
|---|---|
| `CurrentUser(User)` | Signed in — else 401. |
| `MaybeCurrentUser(Option<User>)` | Optional user. |
| `Admin(User)` | Signed in + `role == Admin` — else 403. |
| `RealIp(String)` | Client IP: the `X-Forwarded-For` entry `TRUSTED_PROXY_HOPS` (default 1) from the right, else the socket. |
| `ValidatedForm<T>` | Form body that has passed `T::validate`. |

`extract::session_user` is the root of the user extractors. It verifies the JWT and runs
`User::audit_and_find_by_email` on every signed-in request, which updates
`last_seen_at` / `last_known_ip` and records the day in
`user_daily_activity` (the analytics DAU chart).

### Pagination
Implement `Paginate` with a `table_name()`, then call its two methods:

```rust
impl Paginate for User {
    fn table_name() -> &'static str { "user_details" } // a view is fine
}

User::paginate(&db, &paging).await?; // newest first
User::paginate_where(&db, "WHERE role = $1 ORDER BY last_seen_at DESC", vec!["admin"], &paging).await?;
```

Bindings are positional and passed as `&str`; put `ORDER BY` inside the clause.
Render the Previous / Next links with the `pagination::pager` template macro.
`page` / `page_size` from the request are clamped (`PagingInfo::clamped`, at
most `MAX_PAGE_SIZE` rows).
Only interpolate fixed SQL fragments into the clause, never request data (see
`RoleFilter::sql`).

### Database
- `PrimaryKey` is `i32` (`SERIAL`).
- Every mutable table has `created_at` / `updated_at TIMESTAMPTZ DEFAULT NOW()`
  and a `BEFORE UPDATE` trigger calling `update_updated_at_column()`. Index
  every column you query or join by.
- Users are read through the `user_details` view (joins country / region /
  city). Changing `users` columns means recreating that view in the migration
  and updating the explicit column lists in `domain/user.rs`.
- Prefer `query!` / `query_as!` for static SQL. Use `query_as::<_, T>` with
  `.bind()` only for dynamic SQL.

### shima types in the database
shima 0.8 implements sqlx 0.9's traits, so shima ids can be columns directly
(`User::stripe_customer_id: Option<CustomerId>`, selected as
`stripe_customer_id as "stripe_customer_id: CustomerId"`).

## Admin monitoring

`/admin/monitoring` (admin only, linked from the account menu) shows an
overall health verdict with the problems behind it, plus uptime, memory,
database ping, size and connections, the pool, when the daily tick last
ran, migrations, the largest tables, which env vars are set (values never
shown), the last 24 hours of sign-ups and sign-ins, and recent logs.
`infra/monitoring.rs` keeps the last 1,000 log lines (INFO and up) and the
request counts in memory, so they're per server and start over on restart.
`monitoring::init_tracing` replaces `tracing_subscriber::fmt::init()`, and
the `track_requests` middleware skips `/assets` and the page's own polling.
The checks, the env var list and the 24-hour counts live in
`domain/system_status.rs`.

## Admin user management

All behind `Admin`:

- **`/users`** — search by name, email, or IP; filter by plan and "flagged".
  Rows show verified / locked / flagged status.
- **`/users/{id}`** — the account page: activity strip (last 30 days of
  `user_daily_activity`), account facts, sign-in history (`user_sign_ins`,
  kept 30 days plus the latest), private admin notes (`user_notes`),
  flags (`user_flags`, informational only), and other accounts that
  share one of its IPs. The edit modal changes email
  (swaps the login behind the account), role, lock, and an optional plan end
  date. Deleting a user also deletes their Stripe customer; admins can't be
  deleted.
- **`/admin/analytics`** — user counts with 30-day deltas, the DAU chart
  (Chart.js vendored in `public/scripts/`), and top countries.

Automatic locks: a sign-in from a locked country, or from an IP a locked
account was last seen on, locks a Free account. Paid and admin accounts are
never locked automatically (`locked_after_sign_in`).

### Daily jobs
`POST /_internal/cron/tick` with header `X-Cron-Secret: $CRON_SECRET` runs
`infra/scheduler.rs`: it downgrades lapsed admin-granted plans (emailing
reminders 30 / 7 / 1 days out first) and prunes old sign-ins. A Postgres lease
(`scheduler_locks`) keeps replicas from running it twice. Point a daily cron
at it. New daily jobs go in `run_tick`.

## End-to-end tests (Playwright)

Specs live in `e2e/` and run on every PR (`.github/workflows/playwright.yml`)
against a debug build and a fresh Postgres.

### The test-auth shim
`src/infra/api/auth_test.rs` is mounted only when `ENABLE_TEST_AUTH=true`, and
every route requires `X-Test-Auth-Token` to match `TEST_AUTH_TOKEN`:

- `POST /auth/test/sign-in` — upserts a verified user with the given `role`
  (always applied), records a sign-in from `ip`, and sets the `auth_token`
  cookie. Wrapped by `signInAs(request, context, email, role, { ip })` in
  `e2e/fixtures.ts`; pass `null` as the context to create an account without
  browsing as it.
- `POST /auth/test/sign-in-code` — issues an emailed code and returns the
  plaintext. Wrapped by `issueSignInCode`.

### Writing tests
- `e2e.sh` and the workflow list `playwright+listed-admin@example.com` in
  `ADMIN_EMAILS`, set dummy Google values so the Google button renders, and
  leave Stripe unset so the suite also covers running without billing.
- The database is shared across the suite and never reset. Make every
  address with `testEmail(testInfo, "who")` from `e2e/fixtures.ts`: it adds the
  test id and a per-run suffix, so retries start clean.
- Tests run in parallel, in no fixed order, against that one database. A test
  must only rely on rows it created: never assert on global counts or on a
  list being empty unless the query is narrowed to the test's own accounts.
- Don't lock an account whose `last_known_ip` is `127.0.0.1` — every later
  sign-up from the test runner would be locked by the shared-IP rule. Give
  such accounts their own IP via `signInAs(…, { ip })`.
- Drive setup through the UI when realistic; use the shim when click-throughs
  would be slow or impossible (OAuth, email).

## Accessibility

Target WCAG 2.1 AA. `e2e/accessibility.spec.ts` scans the main pages with
axe and tests keyboard behaviour; add a page to its scan when you add one.

- Every control has a name: visible text, a `<label>`, or `aria-label` for
  icon-only buttons (`&times;` is `aria-label="Close"`).
- Text meets 4.5:1 contrast: `text-gray-500` is the lightest gray for text
  on white; `text-gray-400` fails.
- One `<h1>` per page and no skipped heading levels. Modal partials start
  with an `<h2>`, which names the dialog.
- Decorative images get `alt=""` (a flag next to its country name).
- Anything shown on hover (tooltips, menus) works from the keyboard too, and
  Escape dismisses it.
- Landmarks are labelled when there's more than one of a kind
  (`<nav aria-label="Main">`).

## Conventions for contributions

- New entity → `domain/<name>.rs`; new use-case →
  `use_cases/<aggregate>/<verb>_use_case.rs`; new HTTP surface →
  `infra/api/<resource>.rs` + `templates/<resource>/`.
- `use crate::prelude::*;` at the top of every file in `src/`.
- Comments explain *why*, not *what*. Doc comments on `pub` items whose
  purpose isn't obvious from the signature.
- No `#[allow(...)]` to silence warnings, no speculative abstractions, no
  backwards-compat shims.
- Run `cargo fmt` when you're done.
