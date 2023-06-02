CREATE TYPE user_role as ENUM ('dev', 'admin', 'prefect', 'participant');

ALTER TABLE users
ADD COLUMN permissions user_role NOT NULL DEFAULT 'participant';