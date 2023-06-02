CREATE TABLE
    photos (
        id SERIAL PRIMARY KEY,
        path TEXT NOT NULL,
        event_id INT NOT NULL,
        CONSTRAINT fk_event_id FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE
    )