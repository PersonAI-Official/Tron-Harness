-- Add durable execution history for task runs. Existing task records do not
-- contain enough information to reconstruct sessions from earlier runs.

-- tron:migration:0010_task_execution_records
CREATE TABLE task_execution_records (
    execution_code TEXT PRIMARY KEY,
    task_code TEXT NOT NULL REFERENCES tasks(task_code) ON DELETE CASCADE,
    session_code TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('Running', 'Success', 'Failed')),
    summary TEXT NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_task_executions_task_started
    ON task_execution_records(task_code, started_at DESC, execution_code DESC);

CREATE INDEX idx_task_executions_session
    ON task_execution_records(session_code);
