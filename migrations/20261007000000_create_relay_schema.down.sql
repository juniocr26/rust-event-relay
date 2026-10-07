-- RESTRICT deliberately refuses rollback if objects still depend on this schema.
DROP SCHEMA relay RESTRICT;
