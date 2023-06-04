ALTER TABLE prefect_events ADD COLUMN relation_id SERIAL;
ALTER TABLE participant_events ADD COLUMN relation_id SERIAL;