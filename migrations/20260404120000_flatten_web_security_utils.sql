-- Flatten Web Security: remove intermediate "CSP" util; policies are a direct child of Web Security.
-- Also set util handle to web_security__csp (workspace URL /ws/web_security__csp).
--
-- To undo locally and re-run after editing this file:
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES (8, 'web_security__csp', 'CSP', NULL, 7); UPDATE utils SET handle = 'web_security__csp__policies', name = 'Policies', keywords = 'csp policies content web security', parent_id = 8 WHERE id = 9;"
--   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "DELETE FROM _sqlx_migrations WHERE version = 20260404120000;"
--   sqlx migrate run
--
-- If you already applied the removed migration 20260404130000, clean it up once:
--   DELETE FROM _sqlx_migrations WHERE version = 20260404130000;

UPDATE utils
SET parent_id = 7
WHERE id = 9;

DELETE FROM utils
WHERE id = 8;

UPDATE utils
SET handle   = 'web_security__csp',
    name     = 'CSP',
    keywords = 'csp policies content web security'
WHERE id = 9;
