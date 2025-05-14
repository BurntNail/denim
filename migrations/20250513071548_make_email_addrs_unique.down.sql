-- Add down migration script here

ALTER TABLE users DROP CONSTRAINT unique_emails;