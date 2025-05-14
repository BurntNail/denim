-- Add up migration script here

ALTER TABLE users ADD CONSTRAINT unique_emails UNIQUE (email);