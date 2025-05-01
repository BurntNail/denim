-- Add up migration script here

CREATE TABLE houses (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE forms (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);


CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    first_name TEXT NOT NULL,
    pref_name TEXT,
    surname TEXT NOT NULL,
    email TEXT NOT NULL,
    bcrypt_hashed_password TEXT,
    current_password_is_default BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE staff (
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT staff_user_id
                   FOREIGN KEY (user_id)
                   REFERENCES users(id)
                   ON DELETE CASCADE
);

CREATE TABLE developers
(
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT dev_user_id
        FOREIGN KEY (user_id)
            REFERENCES users (id)
            ON DELETE CASCADE
);

CREATE TABLE students (
    user_id uuid NOT NULL PRIMARY KEY,
    CONSTRAINT student_user_id
        FOREIGN KEY (user_id)
            REFERENCES users (id)
            ON DELETE CASCADE,

    form_id INT,
    CONSTRAINT student_form_id
                      FOREIGN KEY (form_id)
                      REFERENCES forms(id),

    house_id INT,
    CONSTRAINT student_house_id
                      FOREIGN KEY (house_id)
                      REFERENCES houses(id)
);