<div align=center>
<img width="168" height="168" src="https://bytebury.com/assets/images/bibby_welcome.webp" alt="Bytebury's mascot, Bibby" />

<p>A full stack template at the heart of every Bytebury application. Written in Rust. Built with axum, sqlx, htmx, tailwindcss, and askama.</p>
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

## Opt-out Microservices
Bytebury provides a few libraries and microservices that bibby can incorporate into projects. By default these are all included. You can delete any you feel are not important to your needs.

| Name | Description |
| --- | --- |
| geodude | A geolocation microservice built on top of ip2location. Supports auto-updates from ip2location. |
| shima | A simple stripe microservice to help handle payments and integrate customers into your application. |

