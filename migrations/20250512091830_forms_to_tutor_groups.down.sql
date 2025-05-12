ALTER TABLE students DROP COLUMN tutor_group_id;
DROP TABLE tutor_groups;

CREATE TABLE forms (
   id SERIAL PRIMARY KEY,
   name TEXT NOT NULL
);

ALTER TABLE students ADD COLUMN form_id INT NOT NULL default 1;
ALTER TABLE students ADD CONSTRAINT form_id_fk
    FOREIGN KEY (form_id)
        REFERENCES forms(id)
        ON DELETE CASCADE;

ALTER TABLE students ALTER COLUMN house_id DROP NOT NULL;