use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::{Router, extract::State, routing::get};
use dotenvy::dotenv;
use serde::Serialize;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env;
use std::result::Result;

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
    // build our application with a single route
    let app = Router::new()
        .route("/posts", get(list_posts))
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
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
