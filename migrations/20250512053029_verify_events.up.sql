-- Add up migration script here
ALTER TABLE participation ADD COLUMN is_verified BOOL NOT NULL DEFAULT false;