CREATE TABLE bonus_points (
	id SERIAL PRIMARY KEY,
	point_date TIMESTAMP NOT NULL,
    staff_member_id INT,
    CONSTRAINT fk_staff_member_id
        FOREIGN KEY (staff_member_id)
        REFERENCES people(id)
        ON DELETE SET NULL,
    num_points INT NOT NULL,
    reason TEXT NOT NULL
);

CREATE TABLE participant_bonus_points (
	relation_id SERIAL PRIMARY KEY,
	bonus_point_id INT NOT NULL,
        CONSTRAINT fk_bonus_point_id
        FOREIGN KEY (bonus_point_id)
        REFERENCES bonus_points(id)
        ON DELETE CASCADE,
    participant_id INT NOT NULL,
        CONSTRAINT fk_participant_id
        FOREIGN KEY (participant_id)
        REFERENCES people(id)
        ON DELETE CASCADE
);
