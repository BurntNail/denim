-- Add up migration script here
CREATE TABLE events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    date TIMESTAMP NOT NULL,
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
                           ON DELETE CASCADE
)