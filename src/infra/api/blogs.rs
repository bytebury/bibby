use crate::domain::blog::{Blog, BlogForm};
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::api::extract::maybe_current_user::MaybeCurrentUser;
use crate::infra::api::extract::validated_form::ValidatedForm;
use crate::infra::pagination::{Paginate, Paginated, PagingInfo};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{delete, get, patch, put};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/blogs", get(blogs))
        .route("/blogs", put(create_blog))
        .route("/blogs/new", get(new_blog_modal))
        .route("/blogs/{id}", get(view_blog_redirect))
        .route("/blogs/{id}/{slug}", get(view_blog))
        .route("/blogs/{id}", delete(destroy_blog))
        .route("/blogs/{id}", patch(edit_blog))
        .route("/blogs/{id}/edit", get(edit_blog_modal))
}

#[derive(Template, WebTemplate)]
#[template(path = "blogs/blogs.html")]
struct BlogsTemplate {
    shared: SharedContext,
    blogs: Paginated<Blog>,
}

#[derive(Template, WebTemplate)]
#[template(path = "blogs/blog.html")]
struct BlogTemplate {
    shared: SharedContext,
    blog: Blog,
    is_admin: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "blogs/_new_blog.html")]
struct NewBlogModal {}

#[derive(Template, WebTemplate)]
#[template(path = "blogs/_edit_blog.html")]
struct EditBlogModal {
    blog: Blog,
}

async fn blogs(
    State(state): State<SharedState>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    Query(params): Query<PagingInfo>,
) -> Result<BlogsTemplate> {
    Ok(BlogsTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/blogs")
            .with_meta(
                PageMeta::new()
                    .title("Blog")
                    .description("News, notes, and writing from the team."),
            ),
        blogs: Blog::paginate_filter(
            &state.db,
            Some("ORDER BY created_at DESC, id DESC"),
            vec![],
            &params,
        )
        .await?,
    })
}

/// `/blogs/{id}` is the bare-id form. 301 to the slugged URL so crawlers see
/// a single canonical link per post and shared URLs carry the title.
async fn view_blog_redirect(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
) -> Result<impl IntoResponse> {
    let blog = Blog::find_by_id(state.db.as_ref(), id).await?;
    Ok(Redirect::permanent(&format!(
        "/blogs/{}/{}",
        blog.id,
        blog.slug()
    )))
}

async fn view_blog(
    State(state): State<SharedState>,
    Path((id, slug)): Path<(PrimaryKey, String)>,
    MaybeCurrentUser(user): MaybeCurrentUser,
) -> Result<axum::response::Response> {
    let blog = Blog::find_by_id(state.db.as_ref(), id).await?;
    let canonical_slug = blog.slug();
    if slug != canonical_slug {
        return Ok(
            Redirect::permanent(&format!("/blogs/{}/{}", blog.id, canonical_slug)).into_response(),
        );
    }
    let canonical_path = format!("/blogs/{}/{}", blog.id, canonical_slug);
    let is_admin = user.as_ref().map(|u| u.is_admin()).unwrap_or(false);
    Ok(BlogTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path(canonical_path)
            .with_meta(PageMeta::article(&blog)),
        blog,
        is_admin,
    }
    .into_response())
}

async fn new_blog_modal(Admin(_): Admin) -> NewBlogModal {
    NewBlogModal {}
}

async fn create_blog(
    State(state): State<SharedState>,
    Admin(user): Admin,
    headers: HeaderMap,
    ValidatedForm(form): ValidatedForm<BlogForm>,
) -> Result<impl IntoResponse> {
    let blog = Blog::create(state.db.as_ref(), user.id, &form).await?;
    Ok(crate::redirect!(
        format!("/blogs/{}/{}", blog.id, blog.slug()).as_str(),
        &headers
    ))
}

async fn edit_blog_modal(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
) -> Result<impl IntoResponse> {
    let blog = Blog::find_by_id(state.db.as_ref(), id).await?;
    Ok(EditBlogModal { blog }.into_response())
}

async fn edit_blog(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Path(id): Path<PrimaryKey>,
    headers: HeaderMap,
    ValidatedForm(form): ValidatedForm<BlogForm>,
) -> Result<impl IntoResponse> {
    Blog::update(state.db.as_ref(), id, user.id, &form).await?;
    let blog = Blog::find_by_id(state.db.as_ref(), id).await?;
    Ok(crate::redirect!(
        format!("/blogs/{}/{}", blog.id, blog.slug()).as_str(),
        &headers
    ))
}

async fn destroy_blog(
    State(state): State<SharedState>,
    Path(id): Path<PrimaryKey>,
    Admin(_): Admin,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    Blog::destroy(state.db.as_ref(), id).await?;
    Ok(crate::redirect!("/blogs", &headers))
}
