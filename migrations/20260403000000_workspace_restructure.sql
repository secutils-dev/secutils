-- Rename "Home" to "Workspace" and turn it into a group with child items.
UPDATE utils
SET handle   = 'workspace',
    name     = 'Workspace',
    keywords = 'workspace overview home tags secrets scripts'
WHERE id = 1;

-- Add child items under Workspace (id=1).
INSERT INTO utils (id, handle, name, keywords, parent_id)
VALUES (13, 'workspace__overview', 'Overview',
        'home start docs guides changes overview dashboard', 1),
       (14, 'workspace__tags', 'Tags',
        'tags labels categories organize', 1),
       (15, 'workspace__secrets', 'Secrets',
        'secrets keys values environment variables credentials', 1),
       (16, 'workspace__scripts', 'Scripts',
        'scripts deno javascript typescript automation user', 1);
