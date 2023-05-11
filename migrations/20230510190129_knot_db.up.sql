CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    event_name TEXT NOT NULL,
    date TIMESTAMP NOT NULL,
    location TEXT NOT NULL,
    teacher TEXT NOT NULL,
    other_info TEXT
);

CREATE TABLE people (
    id SERIAL PRIMARY KEY,
    person_name TEXT NOT NULL,
    person_email TEXT NOT NULL,
    is_prefect BOOLEAN NOT NULL
);

CREATE TABLE prefect_events (
    prefect_id INT NOT NULL,
    CONSTRAINT fk_prefect_id 
        FOREIGN KEY (prefect_id)
        REFERENCES people(id),

    event_id INT NOT NULL,
    CONSTRAINT fk_event_id
        FOREIGN KEY (event_id)
        REFERENCES events(id)
);

CREATE TABLE participant_events (
    participant_id INT NOT NULL,
    CONSTRAINT fk_participant_id 
        FOREIGN KEY (participant_id)
        REFERENCES people(id),

    event_id INT NOT NULL,
    CONSTRAINT fk_event_id
        FOREIGN KEY (event_id)
        REFERENCES events(id)
);