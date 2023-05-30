CREATE TABLE auth_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    expiry TIMESTAMP
);

CREATE TABLE auth_session_data (
    k TEXT NOT NULL,
    v TEXT NOT NULL,

    session_id TEXT NOT NULL,
    CONSTRAINT fk_session_id
        FOREIGN KEY (session_id)
        REFERENCES auth_sessions(id)
        ON DELETE CASCADE
)