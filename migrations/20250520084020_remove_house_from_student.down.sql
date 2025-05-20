-- Add down migration script here
ALTER TABLE public.students ADD COLUMN house_id INT NOT NULL DEFAULT 1;