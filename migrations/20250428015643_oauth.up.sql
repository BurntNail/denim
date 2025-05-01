-- Add up migration script here
ALTER TABLE users ADD COLUMN access_token TEXT;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    DATA BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);