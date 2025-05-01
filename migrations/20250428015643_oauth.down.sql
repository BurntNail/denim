-- Add down migration script here
ALTER TABLE users DROP COLUMN access_token;
DROP TABLE sessions;