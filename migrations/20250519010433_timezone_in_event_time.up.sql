-- Add up migration script here

ALTER TABLE events ADD COLUMN tz TEXT NOT NULL DEFAULT 'Asia/Bangkok';