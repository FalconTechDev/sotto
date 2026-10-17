-- Provider work must survive process restarts without creating a second financial operation.
-- Lease fields fence workers; provider references identify the result of each external step.
ALTER TABLE billing_transfer_intents
    ADD COLUMN execution_lease_token TEXT,
    ADD COLUMN execution_lease_until TIMESTAMPTZ,
    ADD COLUMN execution_next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN execution_attempt_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN destination_customer_id TEXT,
    ADD COLUMN destination_schedule_id TEXT,
    ADD COLUMN source_schedule_id TEXT,
    ADD COLUMN destination_setup_session_id TEXT,
    ADD COLUMN destination_setup_intent_id TEXT,
    ADD COLUMN last_execution_error_code TEXT;

ALTER TABLE billing_transfer_intents
    ADD CONSTRAINT billing_transfer_execution_lease_pair CHECK (
        (execution_lease_token IS NULL AND execution_lease_until IS NULL)
        OR (execution_lease_token IS NOT NULL AND execution_lease_until IS NOT NULL)
    ),
    ADD CONSTRAINT billing_transfer_execution_attempts_valid CHECK (
        execution_attempt_count >= 0
    ),
    ADD CONSTRAINT billing_transfer_execution_lease_token_not_empty CHECK (
        execution_lease_token IS NULL OR btrim(execution_lease_token) <> ''
    );

CREATE INDEX billing_transfer_execution_due_idx
    ON billing_transfer_intents (updated_at, created_at)
    WHERE state IN ('pending', 'destination_prepared', 'destination_paid',
                    'source_adjustment_pending', 'unknown');
