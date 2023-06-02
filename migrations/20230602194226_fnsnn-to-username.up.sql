ALTER TABLE people ADD COLUMN username TEXT NOT NULL DEFAULT 'Maguire-J-19';

UPDATE people SET username = subquery.surname
FROM (SELECT surname,id FROM people) AS subquery
WHERE people.id = subquery.id;