DROP TABLE sessions;

CREATE TABLE sessions (
    id text primary key not null,
    data bytea not null,
    expiry_date timestamptz not null
);

DROP TABLE secrets;