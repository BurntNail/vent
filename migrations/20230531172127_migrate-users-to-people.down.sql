ALTER TABLE people DROP COLUMN hashed_password;
ALTER TABLE people DROP COLUMN permissions;

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username TEXT NOT NULL,
    hashed_password TEXT NOT NULL
)