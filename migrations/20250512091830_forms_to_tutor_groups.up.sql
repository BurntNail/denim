ALTER TABLE students DROP COLUMN form_id;

DROP TABLE forms;

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

ALTER TABLE students ADD COLUMN tutor_group_id uuid NOT NULL default gen_random_uuid();
ALTER TABLE students ADD CONSTRAINT tutor_group_id_fk
        FOREIGN KEY (tutor_group_id)
        REFERENCES tutor_groups(id)
        ON DELETE CASCADE;

ALTER TABLE students ALTER COLUMN house_id SET NOT NULL;