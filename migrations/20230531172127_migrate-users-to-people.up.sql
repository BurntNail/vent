ALTER TABLE people ADD COLUMN hashed_password TEXT;
ALTER TABLE people ADD COLUMN permissions user_role NOT NULL DEFAULT 'participant';

UPDATE people SET permissions = 'prefect' WHERE is_prefect = true;
UPDATE people SET permissions = 'admin' WHERE form = 'Staff';

DROP TABLE users;