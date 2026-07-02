mod auth;

use askama::Template;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Router, extract::Query, extract::State, routing::get};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env;
use std::result::Result;

#[derive(Template)]
#[template(path = "root.html")]
struct RootTemplate {
    title: &'static str,
}

async fn root() -> impl IntoResponse {
    let root = RootTemplate { title: "title" };
    (StatusCode::OK, Html(root.render().unwrap()))
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

async fn login() -> impl IntoResponse {
    (StatusCode::OK, Html(LoginTemplate.render().unwrap()))
}

async fn auth_login() -> Redirect {
    let client_id = env::var("WORKOS_CLIENT_ID").expect("WORKOS_CLIENT_ID not set");
    let redirect_uri = env::var("WORKOS_REDIRECT_URI").expect("WORKOS_REDIRECT_URI not set");

    let url = url::Url::parse_with_params(
        "https://api.workos.com/user_management/authorize",
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("provider", "authkit"), // hosted AuthKit page: shows Magic Auth + any social you enabled
        ],
    )
    .expect("valid authorize url");

    println!("{url:?}");

    Redirect::to(url.as_str())
}

#[derive(Deserialize)]
struct AuthCallbackCode {
    code: String,
}

async fn auth_callback(Query(AuthCallbackCode { code }): Query<AuthCallbackCode>) -> Redirect {
    println!("{code:?}");
    let user = auth::auth_code_to_user(code).await;
    println!("{user:?}");
    Redirect::to("/")
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(
            env::var("DATABASE_URL")
                .expect("DATABASE_URL not set")
                .as_str(),
        )
        .await
        .unwrap();

    // run any pending migrations on startup (idempotent — sqlx tracks
    // applied migrations in the _sqlx_migrations table)
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations!");

    // build our application with a single route
    let app = Router::new()
        .route("/posts", get(list_posts))
        .route("/login", get(login))
        .route("/", get(root))
        .route("/auth/login", get(auth_login))
        .route("/auth/callback", get(auth_callback))
        .with_state(pool);

    // Render assigns the port via $PORT; fall back to 3000 for local dev
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
struct AuthenticatedUser {
    id: i32,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|t| t.to_str().ok())
            .and_then(|t| t.strip_prefix("Bearer "))
            .and_then(|t| t.parse::<i32>().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(AuthenticatedUser { id })
    }
}
#[derive(Serialize)]
struct Post {
    content: String,
    from: String,
}
async fn list_posts(
    State(pool): State<Pool<Postgres>>,
    AuthenticatedUser { id: user_id }: AuthenticatedUser,
) -> Result<axum::Json<Vec<Post>>, StatusCode> {
    let results = sqlx::query!(
        "SELECT posts.content, users.name
        FROM posts
        INNER JOIN users ON (users.id = posts.posted_by)
        WHERE posted_to = $1
        ORDER BY posts.posted_at DESC
        ",
        user_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("{e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let posts: Vec<Post> = results
        .into_iter()
        .map(|res| Post {
            content: res.content,
            from: res.name,
        })
        .collect();
    Ok(axum::response::Json::from(posts))
}
