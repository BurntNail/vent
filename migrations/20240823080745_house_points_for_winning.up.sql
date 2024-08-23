-- Add up migration script here

ALTER TABLE public.events ADD COLUMN extra_points INTEGER NOT NULL DEFAULT 0;