CREATE TABLE auth_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    expiry TIMESTAMP,
    session_contents TEXT NOT NULL
)