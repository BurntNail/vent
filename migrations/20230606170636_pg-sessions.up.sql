CREATE TABLE sessions (
    id TEXT NOT NULL,
    session_json JSON NOT NULL,
    expires TIMESTAMP
);

CREATE TABLE secrets (
    sekrit BYTEA NOT NULL
)