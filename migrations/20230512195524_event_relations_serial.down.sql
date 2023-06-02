ALTER TABLE prefect_events
DROP COLUMN IF EXISTS relation_id;

ALTER TABLE participant_events
DROP COLUMN IF EXISTS relation_id;