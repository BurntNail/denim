-- Add down migration script here

DROP TABLE sessions, photos;

DROP TABLE participation, events;
DROP TABLE students, staff, admins, tutor_groups, houses;

DROP TABLE users;