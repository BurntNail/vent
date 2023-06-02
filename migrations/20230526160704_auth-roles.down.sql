ALTER TABLE users
DROP COLUMN IF EXISTS permissions;

DROP TYPE user_role;