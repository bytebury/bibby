<div align=center>

<img width="168" height="168" src="https://bytebury.com/assets/images/bibby_welcome.webp" alt="Bytebury's mascot, Bibby" />

<p>
  A full stack template at the heart of every Bytebury application. Written in Rust. Built with axum, sqlx, htmx, tailwindcss, and askama. Out-of-the-box integration with Stripe, OAuth, Postgres, Geolocation, Mailers, Blogs, Announcements, User Management, Privacy and Terms of Service, and more.
</p>
</div>

# Getting Started
You can get started by creating a repository from this template repository. This framework helps the bytebury team deliver efficiently and safely.

## Prerequisites

- [Rust](https://rustup.rs) (the version in `Cargo.toml`'s `rust-version`, or newer)
- [Node.js](https://nodejs.org) and npm, for Tailwind and Playwright
- [PostgreSQL](https://www.postgresql.org), reachable at the `DATABASE_URL` in your `.env`
- `sqlx-cli`, which `./dev.sh` uses to create and migrate the database:

  ```sh
  cargo install sqlx-cli --no-default-features --features postgres,rustls
  ```

For a quick start, you can copy the following lines into your terminal and go to `localhost:8080` to view the application running locally in watch mode.

```sh
git clone https://github.com/bytebury/bibby
cd ./bibby
./dev.sh
```

You can always run the application locally by running the `./dev.sh` file. This will run the application in with a debug build and watch for file changes. This will also watch for html and css changes so that tailwind will also generate.

`./dev.sh` enables the repo's pre-commit hooks automatically for Git checkouts. If you need to enable them manually, run:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook runs `cargo fmt --all` and `cargo sqlx prepare`. If either command changes files, the commit stops so you can review and stage the generated changes before committing again.

## Configuration

`./dev.sh` writes a commented `.env` the first time it runs, and `CLAUDE.md` lists every variable. One deserves a mention here because you can't administer the app without it:

- `ADMIN_EMAILS` (required) — a comma-separated list of email addresses that become admins when they sign in, e.g. `ADMIN_EMAILS=you@example.com,cofounder@example.com`. Whitespace around each address is trimmed. Set it, including in production, before you first sign in.

## Rename after cloning

Bibby's name appears in a handful of places. After creating your repository, replace it in each of these (searching for `bibby`, `Bibby`, and `bytebury` finds them all):

- [ ] `Cargo.toml` — the package (and therefore binary) name `bibby`. `Cargo.lock` follows on the next build, and `src/main.rs` imports the crate as `bibby::…`.
- [ ] `dev.sh` — `cargo watch -x 'run --bin bibby'`, plus the `APP_NAME`, `SEO_DEFAULT_TITLE`, `SEO_DEFAULT_DESCRIPTION`, `WEB_APP_NAME`, and `WEB_APP_SHORT_NAME` defaults it writes to `.env`.
- [ ] `e2e.sh` — `cargo build --bin bibby`, `./target/debug/bibby`, the `bibby-e2e-` log file name, and `APP_NAME="Bibby"`.
- [ ] `.github/workflows/playwright.yml` — `--bin bibby`, `./target/debug/bibby`, `APP_NAME: Bibby`, and the cache `shared-key: bibby-e2e`.
- [ ] `.github/workflows/rust.yml` — the cache keys `bibby-check` and `bibby-sqlx`.
- [ ] `Dockerfile` — the binary path `target/x86_64-unknown-linux-musl/release/bibby`.
- [ ] `package.json` — `name` (`bibby-frontend`) and `description`. Run `npm install` afterwards so `package-lock.json` matches, or `npm ci` in the Docker build fails.
- [ ] `src/lib.rs` and `src/infra/email.rs` — the `"Bibby"` fallback used when `APP_NAME` is unset. (The tests in `src/infra/email.rs` use `"Bibby"` only as sample data.)
- [ ] `e2e/example.spec.ts` — the homepage title check (`/Bibby/i`).
- [ ] `LICENSE` — the copyright holder (`bytebury`).
- [ ] `README.md` — the Bibby mascot image and this introduction.
- [ ] `templates/privacy.html` and `templates/terms.html` — every `[BRACKETED]` placeholder (contact email, effective date, jurisdiction, providers, and so on). Remove the "replace before launch" banner once a lawyer has reviewed them.

## Design Decisions
This framework follows Clean Architecture relatively close. We do not use repositories, as all of our data gathering happens directly on the model itself. This allows for more ergonomic code as seen below.

```rs
pub async fn execute(&self, request: &CreateUser) -> Result<User> {
  match User::find_by_email(self.db.as_ref(), &request.email).await {
      Ok(user) => Ok(user),
      Err(_) => User::create(self.db.as_ref(), &request).await,
  }
}
```

Therefore, we use `use_cases` to derive business logic.

## Common Utilities

### SEO and Web App Metadata
Bibby renders SEO, social sharing, and installable web-app metadata from `SharedContext` in `templates/_layouts/base.html`. The defaults are configured with environment variables, so a new app can ship with correct canonical URLs, Open Graph tags, Twitter cards, a web manifest, and `robots.txt`/`sitemap.xml` without editing templates.

The important defaults are:

```sh
# Public URL used for canonical URLs, Open Graph URLs, robots.txt, and sitemap.xml.
# Falls back to APP_ORIGIN, then http://localhost:${PORT}.
PUBLIC_SITE_URL=https://example.com

SEO_DEFAULT_TITLE="Example App"
SEO_DEFAULT_DESCRIPTION="A short description of the app."
SEO_DEFAULT_IMAGE=/assets/images/app-icon.svg
SEO_TWITTER_HANDLE=@example
SEO_ROBOTS=index,follow

WEB_APP_NAME="Example App"
WEB_APP_SHORT_NAME=Example
WEB_APP_THEME_COLOR="#111827"
WEB_APP_BACKGROUND_COLOR="#f9fafb"
WEB_APP_DISPLAY=standalone

# Off by default to avoid stale caching surprises in server-rendered HTMX apps.
WEB_APP_SERVICE_WORKER=false
```

Root metadata endpoints are built in:

- `/robots.txt`
- `/sitemap.xml`
- `/site.webmanifest`
- `/service-worker.js`

For page-specific metadata, set it in the handler when constructing `SharedContext`:

```rs
use crate::infra::seo::PageMeta;

shared: SharedContext::new(&state)
    .with_user(user)
    .with_canonical_path("/pricing")
    .with_meta(
        PageMeta::new()
            .title("Pricing")
            .description("Choose the plan that fits your team."),
    )
```

For pages that should not be indexed, such as admin screens, auth interstitials, and account billing pages, use:

```rs
shared: SharedContext::new(&state)
    .with_user(Some(user))
    .with_canonical_path("/users")
    .with_meta(PageMeta::new().title("Manage Users").robots("noindex,nofollow"))
```

Blog posts use `PageMeta::article(&blog)`, which sets article structured data, title, excerpt description, image, published date, updated date, and the canonical slug URL. If you add new public content types, add a model method for sitemap data and include those URLs in `sitemap_xml` in `src/infra/api/core.rs`.

### Pagination
You can use the Paginate trait to implement pagination on models.

`table_name` can be a view: `User` paginates over `user_details`, which joins in the user's country, region, and city.

```rs
impl Paginate for User {
    fn table_name() -> &'static str {
        "user_details"
    }
}
```

**Sample Usage**
```rs
User::paginate(&db, &paging).await?; // newest first
User::paginate_where(&db, "WHERE role = $1 ORDER BY last_seen_at DESC", vec!["admin"], &paging).await?;
```

Bindings are positional and passed as `&str`. Only put fixed SQL fragments in the clause, never request data. `page` and `page_size` come from the query string and are clamped (at most 100 rows).

In the template, `{% call pagination::pager(self.users, "/users?") %}{% endcall %}` (from `templates/_macros/pagination.html`) renders the Previous / Next links. Pass a third argument, the element to swap, to page over HTMX instead.

### Macros

Bibby keeps repetition down with a few small macros:

- `redirect!(url, &headers)` and `refresh!(&headers)`: redirect or reload correctly for both plain and HTMX requests (below).
- `static_page!(about, "about.html", "/about", PageMeta::new().title("About"))`: the whole handler for a page that only needs the layout. Route it with `.route("/about", get(about))`.
- `select_user!("WHERE id = $1", id)`: a compile-time-checked `User` query; the column list lives once in `src/domain/user.rs`.
- Template macros: `ui::spinner_button` (`templates/_macros/buttons.html`) and `pagination::pager` (`templates/_macros/pagination.html`).

### Redirects

It is strongly encouraged to use the provided `redirect!` macro when dealing with redirects. Standard browser requests receive a typical 303 Redirect.
HTMX requests receive a 200 OK with the `HX-Redirect` header, preventing HTMX from swapping the redirect target into the current element and forcing a full-page navigation instead.

**Sample Usage**
```rs
async fn sample(headers: HeaderMap) -> impl IntoResponse {
    redirect!("/", &headers)
}
```

## Web Utilities

These are the small, framework-agnostic UI helpers that ship with bibby. They are all driven by plain CSS + a tiny script and are wired into the base layout (`templates/_layouts/base.html`), so you don't have to import anything per-page — just use the markup conventions below.

### Spinners
Every HTMX-driven button gets an in-flight loading spinner for free. There is no JS to call and no class to add: the CSS in `public/styles/tailwind.css` targets the `htmx-request` class that HTMX itself toggles while a request is in flight, transparentizes the label, and paints a CSS-only spinner via `::after`.

A 150ms grace window means quick requests never flash a spinner — only requests that take longer than 150ms render one. The spinner color is per-variant (`--spinner-color`), so contrast holds on every button class.

**Sample Usage**
```html
<!-- Plain HTMX button — spinner is automatic -->
<button hx-post="/users/123/promote" hx-swap="none" class="btn-primary">
  Promote
</button>

<!-- Form submit — `htmx-request` lands on the <form>, the submit button gets the spinner -->
<form hx-post="/users" hx-swap="none">
  <button type="submit" class="btn-primary">Create</button>
</form>
```

For the common HTMX button case, prefer the `spinner_button` macro in `templates/_macros/buttons.html`, which bundles the standard HTMX attribute set:

```html
{% import "_macros/buttons.html" as ui %}

{% call ui::spinner_button("post", "/users/123/promote", "Promote") %}{% endcall %}
{% call ui::spinner_button("delete", url, "Delete user", "btn-danger", confirm_text) %}{% endcall %}
```

### Tooltips
Add `data-tooltip="..."` to any element and `public/scripts/tooltip.js` will position a tooltip below it on hover. For a word or name that explains itself in a tooltip, add `class="tooltip-term"` for a dotted underline. Listeners are delegated from `document`, so it works on swapped-in content too. Keyboard focus shows it as well, Escape hides it, and screen readers read it as the element's description.

The base layout already renders the required `<div id="tooltip" class="tooltip"></div>` element — don't remove it. On mobile (`max-width: 748px`) you can opt-out with `data-tooltip-no-mobile`, which is useful for tap targets where a hover tooltip just gets in the way.

**Sample Usage**
```html
<!-- Basic tooltip -->
<img
  src="/assets/images/flags/us.svg"
  data-tooltip="United States"
  class="h-4 w-5"
/>

<!-- Suppress on mobile -->
<button data-tooltip="Delete this user" data-tooltip-no-mobile class="btn-ghost">
  &times;
</button>
```

Templates already do this for the flag overlay on user avatars (see `templates/users/users.html` and `templates/users/user.html`).

### Modals
Modals are HTMX-driven: any button that targets `#modal` loads a partial into the modal slot, and `public/scripts/modal.js` opens it as an accessible dialog: it is named after the partial's first heading, focus moves to its first field, Tab stays inside it while the page behind is `inert`, and focus returns to the button that opened it. Closing is handled by `closeModal()` (the underlay click, an `&times;` button, the `Escape` key, or a server-emitted `closeModal` event all work).

The base layout already renders the required wrapper:

```html
<div id="modal_wrapper" class="modal wrapper" style="display: none">
  <div class="underlay" onclick="closeModal()"></div>
  <div id="modal" class="content" role="dialog" aria-modal="true" tabindex="-1"></div>
</div>
```

Modal partials should be **content-only** — just the inner card markup, no wrapper. Start with an `<h2>` (it names the dialog) and give the close button `aria-label="Close"`.

**Sample Usage**
```html
<!-- Trigger: any button that swaps a partial into #modal -->
<button hx-get="/users/123/edit" hx-target="#modal" class="btn-link">
  Edit
</button>

<!-- Partial returned by /users/123/edit (content-only) -->
<div class="card-header">
  <h2 class="card-title">Edit user</h2>
  <button type="button" class="btn-ghost" onclick="closeModal()" aria-label="Close">&times;</button>
</div>
<form hx-patch="/users/123" hx-swap="none" class="card-body">
  <!-- ...fields... -->
</form>
```

To close the modal from the server after a successful action, emit the `closeModal` event via `HX-Trigger`:

```rs
([("HX-Trigger", "closeModal")], "").into_response()
```

### Popover Menus
Popover menus use the native `<details>` element with the `menu` class — no extra state to manage. `public/scripts/menu.js` adds two behaviors on top: clicking outside the open menu closes it, clicking an item inside `.menu-items` closes it, and `Escape` closes all open menus. Because it's just a `<details>`, keyboard activation and screen reader semantics come for free.

**Sample Usage**
```html
<details class="menu relative">
  <summary class="btn-ghost cursor-pointer list-none">
    Actions
  </summary>
  <div class="menu-items absolute right-0 mt-1 w-44 rounded-md border border-gray-200 bg-white shadow-lg">
    <button hx-get="/users/123/edit" hx-target="#modal" class="block w-full px-3 py-2 text-left text-sm hover:bg-gray-50">
      Edit
    </button>
    <button hx-delete="/users/123" hx-confirm="Delete this user?" class="block w-full px-3 py-2 text-left text-sm text-red-600 hover:bg-gray-50">
      Delete
    </button>
  </div>
</details>
```

Note: the tooltip script automatically suppresses hover tooltips on `details.menu > summary` so a tooltip doesn't pop while the menu is opening.

## Feature Toggles
Admins switch features on for everyone at `/admin/feature-toggles`. A toggle is on or off and can carry a value. The first one is **Google Analytics**, off by default: turn it on with your Google tag ID (`G-XXXXXXXXXX`) and gtag loads on every page, and the privacy policy mentions its cookies.

Features are declared in code (`Feature` in `src/domain/feature_toggle.rs`) and stored in the `feature_toggles` table. Each page reads them with the announcement banner (`shared.site.toggles`), so a change shows everywhere as soon as it is saved.

## OAuth Providers
Google OAuth is supported out of the box, alongside email sign-up and sign-in with emailed one-time codes. `src/infra/auth/google.rs` is three functions: `is_configured`, `auth_url` and `exchange_code`. The routes are written out in `src/infra/api/auth.rs`: `/auth/google` starts the flow and `/auth/google/callback` finishes it.

To add another provider:

1. Add `src/infra/auth/<provider>.rs` with the same three functions, reading its client id, secret, and callback URL from the environment.
2. Add a start route and a callback route for it in `src/infra/api/auth.rs`, following the Google handlers (they set and check the `oauth_nonce` cookie and the signed `OAuthState`).
3. If accounts should record which provider created them, add a variant to `AuthProvider` in `src/domain/user.rs` and widen the `users.auth_provider` check constraint in a new migration.
4. Add a button for it next to `templates/_partials/google_button.html`.

## Opt-out Microservices
Bytebury provides a few libraries and microservices that bibby can incorporate into projects. By default these are all included. You can delete any you feel are not important to your needs. Bytebury uses Railway as our primary cloud host, so you may see some preference there, but all microservices are platform / cloud-provider agnostic.

| Name | Description |
| --- | --- |
| geodude | A geolocation microservice built on top of ip2location. Supports auto-updates from ip2location. |
| mailman | A mailer microservice. Send e-mails on your behalf. |

## Listening to Stripe Webhooks
```sh
stripe listen --forward-to localhost:8080/stripe
```

# Tests
We strongly encourage you to test mission-critical features through end-to-end (e2e) tests. Bibby uses Playwright for this, which can be run using `./e2e.sh`. We also bootstrap GitHub Actions, which will automatically execute tests and e2e tests on every Pull Request as the default functionality.
