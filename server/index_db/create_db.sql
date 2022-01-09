CREATE TABLE IF NOT EXISTS stripe_customers
(
    customer_id       TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS stripe_subscriptions
(
    subscription_id   TEXT PRIMARY KEY,
    customer_id       TEXT NOT NULL,
    active            BOOLEAN NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_stripe_subscriptions_customer_id FOREIGN KEY (customer_id) REFERENCES stripe_customers (customer_id) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE IF NOT EXISTS stripe_payment_methods
(
    payment_method_id TEXT PRIMARY KEY,
    customer_id       TEXT NOT NULL,
    last_4            TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_stripe_payment_methods_customer_id FOREIGN KEY (customer_id) REFERENCES stripe_customers (customer_id) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE IF NOT EXISTS apple_payees
(
    id BIGSERIAL NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS google_payees
(
    id BIGSERIAL NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS account_tiers
(
    id                 BIGSERIAL PRIMARY KEY,
    bytes_cap          BIGINT NOT NULL,
    valid_until        TIMESTAMPTZ,
    stripe_customer_id TEXT,
    payee_apple        BIGINT,
    payee_google       BIGINT,
    CONSTRAINT fk_account_tiers_payee_stripe_stripe_customer_id FOREIGN KEY (stripe_customer_id) REFERENCES stripe_customers (customer_id) DEFERRABLE INITIALLY DEFERRED,

    CONSTRAINT fk_account_tiers_payee_apple_apple_payees_id FOREIGN KEY (payee_apple) REFERENCES apple_payees (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT fk_account_tiers_payee_google_google_payees_id FOREIGN KEY (payee_google) REFERENCES google_payees (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT at_most_one_payment_method CHECK (
        NOT (
                (stripe_customer_id IS NOT NULL AND payee_apple IS NOT NULL) AND
                (stripe_customer_id IS NOT NULL AND payee_google IS NOT NULL) AND
                (payee_apple IS NOT NULL AND payee_google IS NOT NULL) AND
                (stripe_customer_id IS NOT NULL AND payee_apple IS NOT NULL AND payee_google IS NOT NULL
                    )
            )
        )
);

CREATE TABLE IF NOT EXISTS accounts
(
    public_key   TEXT   NOT NULL PRIMARY KEY,
    name         TEXT   NOT NULL,
    account_tier BIGINT NOT NULL,
    CONSTRAINT fk_accounts_account_tier_account_tiers_id FOREIGN KEY (account_tier) REFERENCES account_tiers (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT uk_name UNIQUE (name) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE IF NOT EXISTS files
(
    id                TEXT    NOT NULL PRIMARY KEY,
    parent            TEXT    NOT NULL,
    parent_access_key TEXT    NOT NULL,
    is_folder         BOOLEAN NOT NULL,
    name_encrypted    TEXT    NOT NULL,
    name_hmac         TEXT    NOT NULL,
    owner             TEXT    NOT NULL,
    deleted           BOOLEAN NOT NULL,
    metadata_version  BIGINT  NOT NULL,
    content_version   BIGINT  NOT NULL,
    document_size     BIGINT,
    CONSTRAINT fk_files_parent_files_id FOREIGN KEY (parent) REFERENCES files (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT fk_files_owner_accounts_public_key FOREIGN KEY (owner) REFERENCES accounts (public_key) DEFERRABLE INITIALLY DEFERRED,
    -- the CASE WHEN ... THEN TRUE ELSE NULL END adds a nullable 3rd dervied column to what's essentially a unique constraint
    -- because null != null, values where the CASE expression are true are effectively ignored by the constraint
    CONSTRAINT uk_files_name_parent EXCLUDE (parent WITH =, name_hmac WITH =, (CASE WHEN (NOT deleted AND id != parent) THEN TRUE ELSE NULL END) WITH =) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT documents_must_have_size CHECK (is_folder OR document_size IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS user_access_keys
(
    id            BIGSERIAL NOT NULL PRIMARY KEY,
    file_id       TEXT NOT NULL,
    sharee        TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    CONSTRAINT uk_user_access_keys UNIQUE (file_id, sharee) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT fk_user_access_keys_file_id_files_id FOREIGN KEY (file_id) REFERENCES files (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT fk_user_access_keys_sharee_id_accounts_public_key FOREIGN KEY (sharee) REFERENCES accounts (public_key) DEFERRABLE INITIALLY DEFERRED
);
