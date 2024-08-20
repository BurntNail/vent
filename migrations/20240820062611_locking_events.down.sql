-- Add down migration script here
ALTER TABLE public.events DROP COLUMN is_locked;