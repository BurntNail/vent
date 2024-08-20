-- Add up migration script here
ALTER TABLE public.events ADD COLUMN is_locked BOOLEAN NOT NULL DEFAULT FALSE;