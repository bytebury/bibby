# AGENTS.md

Guidance for AI coding agents (Claude, Cursor, Copilot, etc.) working in this repo. Read this end-to-end before making
changes. The conventions here are load-bearing — violating them produces builds that compile but break at runtime, or
PRs that get rejected.

## Project overview

Bibby is the Bytebury full-stack template, written in Rust. Stack:

- **axum 0.8** — HTTP framework
- **sqlx 0.9** with PostgreSQL — database (note: `0.9`, not `0.8` — see [shima caveat](#shimasqlx-version-mismatch))
- **askama 0.16** + `askama_web` — server-rendered templates
- **htmx** — interactivity without an SPA
- **tailwindcss v4** — styling
- **Playwright** — e2e tests (`./e2e.sh`)

Out-of-the-box integrations: Stripe, OAuth (Google by default), Postgres, Geolocation (`geodude`), Mailers (`paperboy`),
Blogs, Announcements, User Management, Privacy/ToS pages.

Run locally with `./dev.sh` (debug build, file-watch for Rust + HTML + CSS).

## Architecture: Clean Architecture, no repositories

The project follows Clean Architecture, but **data access lives on the model itself** rather than in a repository layer.
Business logic lives in `use_cases`. Concretely:

```
src/
├── domain/          # Models + value objects. Models own their own SQL.
│   ├── user.rs      # impl User { pub async fn create(...), find_by_email(...), ... }
│   └── value_objects/
├── use_cases/       # Business logic. One struct per use case, with .execute(request).
│   └── user/register_user_use_case.rs
├── infra/           # HTTP handlers, auth, db, payments, pagination, etc.
│   ├── api/         # axum handlers + extractors
│   ├── auth/
│   └── db.rs
├── error.rs
├── prelude.rs       # Re-exports for every module
└── main.rs
```

Idiomatic example (from the README):

```rust
pub async fn execute(&self, request: &CreateUser) -> Result<User> {
    match User::find_by_email(self.db.as_ref(), &request.email).await {
        Ok(user) => Ok(user),
        Err(_) => User::create(self.db.as_ref(), &request).await,
    }
}
```

**Do not introduce a repository layer.** If you find yourself wanting one, the right answer is a new async method on the
model.

### Use case shape

Each use case is its own struct (`FooUseCase`), constructed with whatever infra it needs (typically `SharedDatabase`),
and exposes a single `pub async fn execute(&self, request: &Request) -> Result<T>`. Group related use cases under a
parent struct (e.g. `UserUseCases { register, search }`) so they can be wired through `SharedState`.

## Idiomatic Rust

- Prefer `?` over `match` for error propagation. Use `match` only when both arms do meaningful work (the README example
  is one of those — both arms return `Ok`).
- Return `Result<T>` (the alias in `error.rs`), not `Result<T, Box<dyn Error>>` or ad-hoc error enums per module.
- `use crate::prelude::*;` at the top of every file in `src/`. The prelude re-exports `SharedState`, `error::*`,
  `PrimaryKey`, askama traits, chrono types, serde derives, `sqlx::prelude::*`, and `Display`/`Formatter`. Don't
  re-import these individually.
- Don't `clone()` to dodge a borrow checker error without thinking. Look at lifetimes first.
- Don't add `#[allow(...)]` to silence warnings — fix the warning.
- Don't add error handling, retries, or fallbacks for cases that can't happen. Trust framework guarantees and internal
  invariants. Validate only at system boundaries (HTTP input, external APIs).
- Don't write comments that restate what the code already says. Comments are for *why* — hidden constraints, subtle
  invariants, workarounds for specific bugs. If removing the comment wouldn't confuse a future reader, don't write it.
- Doc comments (`///`) on `pub` items are welcome when the function's purpose isn't obvious from the signature (see
  `RegisterUserUseCase::execute` for an example).

## Newtype / value object pattern

Use the newtype pattern for any primitive that carries domain meaning. Live examples in `src/domain/value_objects/`:

- `Severity` — enum stored as lowercase TEXT, with `sqlx::Type`, serde, and `Display`.
- `Toggle` — wraps `Option<String>` so handlers can treat a missing HTML checkbox as `false` instead of a parse error.
- `Markdown` — wraps a markdown string with a sanitization/render boundary.

When introducing a new value object:

1. Put it in `src/domain/value_objects/` and add it to `mod.rs`.
2. Derive what the surrounding code needs: `Debug, Clone, Deserialize, Serialize`, and `sqlx::Type` if it's persisted.
3. Implement `Display` (and `From` / `TryFrom` where conversion is meaningful).
4. For enums persisted as TEXT, use `#[sqlx(rename_all = "lowercase", type_name = "TEXT")]` and a matching
   `#[serde(rename_all = "lowercase")]`.
5. Prefer parse-don't-validate: if `Email` is a value object, the constructor returns `Result<Email>` and downstream
   code never has to re-check.

A raw `String` in a struct field is a code smell when the string has invariants (an email, a slug, a Stripe id). Wrap
it.

## Critical, non-obvious conventions

These are the things that compile fine but break at runtime, or get caught in code review every time.

### Redirects always go through `redirect!`

Every handler redirect must use the `redirect!(url, &headers)` macro from `src/infra/api/mod.rs`. **Never** call
`axum::response::Redirect::to(...)` directly.

```rust
async fn sample(headers: HeaderMap) -> impl IntoResponse {
    redirect!("/", &headers)
}
```

**Why:** the macro inspects `HX-Request` and returns an `HX-Redirect` response for HTMX callers (falling back to a 303).
`Redirect::to` causes HTMX to swap the redirect target into the triggering element instead of doing a full-page nav.

**Implication:** every handler that *may* redirect must accept a `HeaderMap` parameter — including pure GET pages that
gate on auth/role and bounce to `/` or `/upgrade`. There is no "this redirect can't be HTMX" exception.

### Timestamps render through `<time data-utc>`

Every displayed timestamp in an askama template must render as:

```html

<time data-utc="{{ x.created_at }}"></time>
```

(empty body — `public/scripts/timezones.js` fills `textContent` on load and after `htmx:afterSwap`.) Never print a
`DateTime<Utc>` directly into visible page content.

Exceptions:

- Hidden form inputs (`<input type="hidden" name="last_seen_at" value="{{ ... }}">`) — stay raw so they round-trip to
  the server.
- Public/SEO blog pages may use `<time datetime="{{ x.to_rfc3339() }}">{{ x.display_date() }}</time>` for
  server-rendered text.

### Tooltip container must be present

If a layout loads `public/scripts/tooltip.js`, it must render `<div id="tooltip" class="tooltip"></div>` once near the
end of `<body>`, and ship matching `.tooltip` / `.tooltip.show` CSS. The script depends on the implicit DOM global
`tooltip`; without that element, the first hover throws `ReferenceError: tooltip is not defined`.

Don't rename the id. `templates/_layouts/base.html` already does this — keep it.

### Modal wrapper structure is fixed

Any layout that loads `modal.js` must render this triplet near the end of `<body>`:

```html

<div id="modal_wrapper" class="modal wrapper" style="display: none">
    <div class="underlay" onclick="closeModal()"></div>
    <div id="modal" class="content"></div>
</div>
```

Plus matching CSS for `.modal.wrapper`, `.modal.wrapper > .content`, `.modal.wrapper > .underlay`,
`.modal.wrapper.closing`, and `fadeIn` / `fadeOut` / `zoomOut` keyframes.

Modal partials returned to HTMX must be **content-only** (just the inner card) — no outer fixed/overlay div. Use
`onclick="closeModal()"` on close buttons, not custom DOM removal. To close from the server after a successful action,
emit:

```rust
([("HX-Trigger", "closeModal")], "").into_response()
```

### Askama macro quirks (0.16)

1. **`{% call %}` is a BLOCK tag.** It must always be paired with `{% endcall %}`, even for macros with no caller body:
   ```
   {% call ui::spinner_button("delete", "/x", "Go") %}{% endcall %}
   ```
   Omitting `{% endcall %}` makes askama swallow the rest of the template into the call body and produces confusing
   errors like `node 'when' was not expected in the current context: 'call' block`.

2. **`self.` is NOT auto-applied inside macro args, `{% let %}` RHS, or any other Rust expression** — only inside
   `{{ ... }}` interpolation:
   ```
   {{ user.id }}                                       ✓ resolves to self.user.id
   {% call ui::btn(format!("/u/{}", user.id)) %}       ✗ "cannot find value `user`"
   {% call ui::btn(format!("/u/{}", self.user.id)) %}  ✓
   {% let path = format!("/u/{}", self.user.id) %}     ✓
   ```
   For dynamic URLs / confirm prompts, build them with `{% let path = format!(..., self.field) %}` first, then pass
   `path.as_str()` to the macro. The call site reads better too.

If you see `cannot find value 'x'` originating from a `Template` derive after editing a template, the fix is almost
always to prefix with `self.` inside whatever expression you just wrote.

### shima/sqlx version mismatch

`shima 0.6.x` derives `sqlx::Type` against **sqlx 0.8 only**. Bibby is on sqlx **0.9**. If you put `shima::CustomerId` (
or any other shima value object) directly on a struct that derives `FromRow`, the build breaks with two distinct
`sqlx_core` versions in the dep tree.

**Workaround until shima publishes a sqlx 0.9 build:** store shima-typed columns as `Option<String>` on the model (see
`User::stripe_customer_id`) and convert with `CustomerId::try_from(...)` at the use-case boundary. Do **not** copy the
`Option<CustomerId>` pattern from theschoolbank into bibby.

### dev.sh tailwind watch flag

The tailwind watcher in `dev.sh` must use `--watch=always`, not plain `--watch`. Tailwind v4's default `--watch` exits
when stdin closes, and IDE run consoles (RustRover) close the script's stdin — so the watcher does the initial build,
dies silently, and `styles.css` goes stale.

Keep `cargo-watch` in the foreground in `dev.sh` (don't background it with `wait -n` — it terminates immediately without
a TTY).

## Web utilities cheat sheet

All wired into `templates/_layouts/base.html` — just use the markup conventions, no per-page imports.

### Spinners

Every HTMX-driven button gets a CSS-only loading spinner via `htmx-request` (toggled by HTMX itself). 150ms grace
window. Spinner color is per-variant via `--spinner-color`.

For the common case, prefer the `spinner_button` macro:

```html
{% import "_macros/buttons.html" as ui %}
{% call ui::spinner_button("post", "/users/123/promote", "Promote") %}{% endcall %}
{% call ui::spinner_button("delete", url, "Delete user", "btn-danger", confirm_text) %}{% endcall %}
```

### Tooltips

`data-tooltip="..."` on any element. Opt out on mobile with `data-tooltip-no-mobile`. Re-initialized after every
`htmx:afterSwap`.

### Modals

HTMX-driven. Trigger: `hx-get="..." hx-target="#modal"`. Close: `closeModal()` from a button, underlay click, `Escape`,
or server-emitted `HX-Trigger: closeModal`.

### Popover menus

Use native `<details class="menu">`. `menu.js` handles outside-click, item-click-closes, and `Escape`. Keyboard + a11y
come from `<details>`.

## Pagination

Implement the `Paginate` trait on a model and use the macros:

```rust
impl Paginate for User {
    fn table_name() -> &'static str { "users" }
}

paginate!(User, &db);
paginate_with!(User, &db, "where role = $1", vec!["admin"]);
```

## OAuth

Google is wired by default. To add a provider, extend the `OAuthProvider` enum and add its config — endpoints
`/auth/{provider_code}` and `/auth/{provider_code}/callback` are generated automatically.

## Stripe

```sh
stripe listen --forward-to localhost:8080/stripe
```

`STRIPE_WEBHOOK_SECRET` lives in `.env`.

## Microservices (opt-out)

Bytebury ships two microservices that bibby integrates with out of the box. Delete what you don't need — they're
platform-agnostic.

| Name     | Description                                                                    |
|----------|--------------------------------------------------------------------------------|
| geodude  | Geolocation microservice on top of ip2location. Auto-updates from ip2location. |
| paperboy | Mailer microservice.                                                           |

## Tests

End-to-end tests are Playwright, run via `./e2e.sh`. GitHub Actions runs unit + e2e on every PR. Mission-critical
features should have e2e coverage.

## Workflow expectations for agents

- Edit existing files; don't create new ones unless the task requires it.
- Don't write `*.md` documentation files (including planning docs, decision logs, summaries) unless the user asks for
  them.
- Don't introduce abstractions, helpers, or feature flags for hypothetical future requirements. Three similar lines
  beats a premature abstraction.
- Don't add backwards-compat shims (renamed `_var`, re-exports, `// removed: ...` comments) — if it's unused, delete it.
- For UI/frontend changes, run `./dev.sh` and exercise the feature in a browser before claiming it works.
  Type-checking ≠ feature-correctness.
- For risky/destructive actions (force push, `reset --hard`, dropping tables, deleting branches), stop and confirm with
  the user.
- Prefer creating a new commit over amending. Never skip hooks (`--no-verify`) without explicit permission.
- ALWAYS run cargo fmt when you are done making changes.