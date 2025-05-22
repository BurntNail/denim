-- Add up migration script here
CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    first_name TEXT NOT NULL,
    pref_name TEXT,
    surname TEXT NOT NULL,
    email TEXT NOT NULL,
    bcrypt_hashed_password TEXT,
    access_token TEXT,
    current_password_is_default BOOLEAN NOT NULL DEFAULT FALSE,

    CONSTRAINT unique_emails UNIQUE(email)
);

CREATE TABLE staff (
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT staff_user_id
        FOREIGN KEY (user_id)
            REFERENCES users(id)
            ON DELETE CASCADE
);

CREATE TABLE admins
(
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT dev_user_id
        FOREIGN KEY (user_id)
            REFERENCES users (id)
            ON DELETE CASCADE
);

CREATE TABLE houses (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE tutor_groups (
    id uuid PRIMARY KEY default gen_random_uuid(),
    staff_id uuid NOT NULL,
    house_id INT NOT NULL,

    CONSTRAINT staff_id_fk
        FOREIGN KEY (staff_id)
            REFERENCES staff(user_id)
            ON DELETE CASCADE,

    CONSTRAINT house_id_fk
        FOREIGN KEY (house_id)
            REFERENCES houses(id)
            ON DELETE CASCADE
);

CREATE TABLE students (
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT student_user_id
        FOREIGN KEY (user_id)
            REFERENCES users (id)
            ON DELETE CASCADE,

    tutor_group_id UUID NOT NULL,
    CONSTRAINT tutor_group_fk
        FOREIGN KEY (tutor_group_id)
            REFERENCES tutor_groups(id)
);

CREATE TABLE events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    date TIMESTAMP NOT NULL,
    tz TEXT NOT NULL,
    location TEXT,
    extra_info TEXT,

    associated_staff_member uuid,
    CONSTRAINT staff_fk
        FOREIGN KEY (associated_staff_member)
            REFERENCES staff(user_id)
);

CREATE TABLE participation (
    event_id uuid NOT NULL,
    CONSTRAINT event_id_fk
        FOREIGN KEY (event_id)
            REFERENCES events(id)
            ON DELETE CASCADE,

    student_id uuid NOT NULL,
    CONSTRAINT student_id_fk
        FOREIGN KEY (student_id)
            REFERENCES students(user_id)
            ON DELETE CASCADE,

    is_verified BOOL NOT NULL DEFAULT FALSE
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    DATA BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);