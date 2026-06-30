-- Add migration script here
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE posts (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    posted_to INTEGER NOT NULL REFERENCES users(id),
    posted_by INTEGER NOT NULL REFERENCES users(id),
    posted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT posts_cannot_post_to_self CHECK (posted_by <> posted_to)
);
