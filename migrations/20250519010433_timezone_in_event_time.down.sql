-- Add down migration script here

ALTER TABLE events DROP COLUMN tz;
