ALTER TABLE people
ADD COLUMN is_prefect BOOL NOT NULL DEFAULT 'false';

UPDATE people
SET
    is_prefect = true
WHERE
    permissions = 'prefect';