-- Add per-automation serial or parallel execution behavior.

-- tron:migration:0011_automation_concurrency_mode
ALTER TABLE automations
    ADD COLUMN concurrency_mode TEXT NOT NULL DEFAULT 'Serial'
    CHECK (concurrency_mode IN ('Serial', 'Parallel'));
