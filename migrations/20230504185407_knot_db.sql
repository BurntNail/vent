CREATE TABLE IF NOT EXISTS events (
    name TEXT NOT NULL,
    date TIMESTAMP NOT NULL,
    location TEXT NOT NULL,
    teacher TEXT NOT NULL,
    prefects TEXT NOT NULL,
    participants TEXT NOT NULL,
    other_info TEXT
)