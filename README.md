<div align=center>

<img width="168" height="168" src="https://bytebury.com/assets/images/bibby_welcome.webp" alt="Bytebury's mascot, Bibby" />

<p>
  A full stack template at the heart of every Bytebury application. Written in Rust. Built with axum, sqlx, htmx, tailwindcss, and askama. Out-of-the-box integration with Stripe, OAuth, Postgres, Geolocation, Mailers, Blogs, Announcements, User Management, Privacy and Terms of Service, and more.
</p>
</div>

# Getting Started
You can get started by creating a repository from this template repository. This framework helps the bytebury team deliver efficiently and safely. You will need Rust and NodeJS available in your development environment to run the application.

For a quick start, you can copy the following lines into your terminal and go to `localhost:8080` to view the application running locally in watch mode.

```sh
git clone https://github.com/bytebury/bibby
cd ./bibby
./dev.sh
```

You can always run the application locally by running the `./dev.sh` file. This will run the application in with a debug build and watch for file changes. This will also watch for html and css changes so that tailwind will also generate.

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

### Pagination
You can use the Paginate trait to implement pagination on models.

```rs
impl Paginate for User {
    fn table_name() -> &'static str {
        "users"
    }
}
```

**Sample Usage**
```rs
paginate!(User, &db);
paginate_with!(User, &db, "where role = $1", vec!["admin"]);
```

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
Add `data-tooltip="..."` to any element and `public/scripts/tooltip.js` will position a tooltip below it on hover. The tooltip is re-initialized after every `htmx:afterSwap`, so it works on swapped-in content too.

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

Templates already do this for the flag overlay on user avatars (see `templates/users/_edit_user.html`).

### Modals
Modals are HTMX-driven: any button that targets `#modal` loads a partial into the modal slot, and `public/scripts/modal.js` flips the wrapper to `display: flex` on `htmx:afterSwap`. Closing is handled by `closeModal()` (the underlay click, an `&times;` button, the `Escape` key, or a server-emitted `closeModal` event all work).

The base layout already renders the required wrapper:

```html
<div id="modal_wrapper" class="modal wrapper" style="display: none">
  <div class="underlay" onclick="closeModal()"></div>
  <div id="modal" class="content"></div>
</div>
```

Modal partials should be **content-only** — just the inner card markup, no wrapper. Use `onclick="closeModal()"` for the close button.

**Sample Usage**
```html
<!-- Trigger: any button that swaps a partial into #modal -->
<button hx-get="/users/123/edit" hx-target="#modal" class="btn-link">
  Edit
</button>

<!-- Partial returned by /users/123/edit (content-only) -->
<div class="card-header">
  <h2 class="card-title">Edit user</h2>
  <button type="button" class="btn-ghost" onclick="closeModal()">&times;</button>
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

## OAuth Providers
By default, we support Google OAuth out of the box. If you'd like to support other OAuth clients, you will need to add your new provider into the `OAuthProvider` enum and add a configuration for it. This will automatically set up the auth endpoints `/auth/{provider_code}` and the callback `/auth/{provider_code}/callback`.

## Opt-out Microservices
Bytebury provides a few libraries and microservices that bibby can incorporate into projects. By default these are all included. You can delete any you feel are not important to your needs. Bytebury uses Railway as our primary cloud host, so you may see some preference there, but all microservices are platform / cloud-provider agnostic.

| Name | Description |
| --- | --- |
| geodude | A geolocation microservice built on top of ip2location. Supports auto-updates from ip2location. |
| paperboy | A mailer microservice. Send e-mails on your behalf. |

## Listening to Stripe Webhooks
```sh
stripe listen --forward-to localhost:8080/stripe
```

# Tests
We strongly encourage you to test mission-critical features through end-to-end (e2e) tests. Bibby uses Playwright for this, which can be run using `./e2e.sh`. We also bootstrap GitHub Actions, which will automatically execute tests and e2e tests on every Pull Request as the default functionality.

