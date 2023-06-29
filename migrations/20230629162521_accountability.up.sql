ALTER TABLE participant_events
ADD COLUMN is_verified BOOL NOT NULL DEFAULT 'true';

ALTER TABLE photos
ADD COLUMN added_by INTEGER;
ALTER TABLE photos ADD CONSTRAINT added_by_fk_constraint FOREIGN KEY (added_by) REFERENCES people (id) ON DELETE CASCADE