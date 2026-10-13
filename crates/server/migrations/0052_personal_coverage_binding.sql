-- Give every personal Checkout a durable opaque identity that Stripe can echo on invoices.
-- Existing accounts receive distinct references without exposing their user or operation ids.
ALTER TABLE billing_personal_accounts
    ADD COLUMN allocation_reference TEXT NOT NULL
        DEFAULT ('alloc_' || gen_random_uuid()::TEXT),
    ADD CONSTRAINT billing_personal_accounts_allocation_reference_key
        UNIQUE (allocation_reference),
    ADD CONSTRAINT billing_personal_accounts_allocation_reference_nonempty
        CHECK (btrim(allocation_reference) <> '');
