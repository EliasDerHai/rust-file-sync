-- The pwa app client is gone. Reattribute any of its events to the web client first so the
-- delete cannot trip the file_event FK (no ON DELETE clause, i.e. it would block).
UPDATE file_event SET client_id = 'c3d4e5f6-7a8b-4c9d-8e2f-1a3b5c7d9e0f' WHERE client_id = 'f4a7b3c2-8d5e-4f6a-9b2c-1e3d5f7a9b0c';
DELETE FROM client WHERE id = 'f4a7b3c2-8d5e-4f6a-9b2c-1e3d5f7a9b0c';
