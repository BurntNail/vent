ALTER TABLE people DROP COLUMN hashed_password IF EXISTS;
ALTER TABLE people DROP COLUMN user_role IF EXISTS;

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username TEXT NOT NULL,
    hashed_password TEXT NOT NULL
)