CREATE TABLE rewards (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    first_entry_pts INT NOT NULL,
    second_entry_pts INT NOT NULL
);

INSERT INTO public.rewards
    ("name", first_entry_pts, second_entry_pts)
    VALUES('Tie', 6, 4);
INSERT INTO public.rewards
    ("name", first_entry_pts, second_entry_pts)
    VALUES('Mug', 20, 10);

CREATE TABLE rewards_received (
    reward_id INT NOT NULL,
    CONSTRAINT reward_fk
        FOREIGN KEY (reward_id)
        REFERENCES rewards(id)
        ON DELETE CASCADE,

    person_id INT NOT NULL,
    CONSTRAINT person_fk
        FOREIGN KEY (person_id)
        REFERENCES people(id)
        ON DELETE CASCADE
);

ALTER TABLE people ADD COLUMN was_first_entry BOOLEAN NOT NULL DEFAULT 'true';