CREATE TABLE users (
    id SERIAL,
    username TEXT NOT NULL,
    hashed_password TEXT NOT NULL
)