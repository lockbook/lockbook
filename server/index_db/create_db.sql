CREATE TABLE IF NOT EXISTS stripe_payees
(
    payee_id bigserial NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS apple_payees
(
    payee_id bigserial NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS google_payees
(
    payee_id bigserial NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS account_tiers
(
    account_tier_id bigserial PRIMARY KEY,
    bytes_cap       bigint NOT NULL,
    valid_until     timestamptz,
    payee_id_stripe bigint REFERENCES stripe_payees (payee_id),
    payee_id_apple  bigint REFERENCES apple_payees (payee_id),
    payee_id_google bigint REFERENCES google_payees (payee_id),
    CONSTRAINT at_most_one_payment_method CHECK (
        NOT (
                (payee_id_stripe IS NOT NULL AND payee_id_apple IS NOT NULL) AND
                (payee_id_stripe IS NOT NULL AND payee_id_google IS NOT NULL) AND
                (payee_id_apple IS NOT NULL AND payee_id_google IS NOT NULL) AND
                (payee_id_stripe IS NOT NULL AND payee_id_apple IS NOT NULL AND payee_id_google IS NOT NULL)
            )
        )
);

CREATE TABLE IF NOT EXISTS accounts
(
    name         text   NOT NULL,
    public_key   text   NOT NULL,
    account_tier bigint NOT NULL REFERENCES account_tiers (account_tier_id),
    CONSTRAINT pk_accounts PRIMARY KEY (name),
    CONSTRAINT uk_public_key UNIQUE (public_key)
);

CREATE TABLE IF NOT EXISTS files
(
    id                text    NOT NULL,
    parent            text    NOT NULL,
    parent_access_key text    NOT NULL,
    is_folder         boolean NOT NULL,
    name              text    NOT NULL,
    owner             text    NOT NULL,
    signature         text    NOT NULL,
    deleted           boolean,
    metadata_version  bigint  NOT NULL,
    content_version   bigint  NOT NULL,
    CONSTRAINT pk_files PRIMARY KEY (id),
    CONSTRAINT fk_files_parent_files_id FOREIGN KEY (parent) REFERENCES files (id),
    CONSTRAINT fk_files_owner_accounts_name FOREIGN KEY (owner) REFERENCES accounts (name)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_files_name_parent ON files (parent, name) WHERE (NOT deleted);

CREATE TABLE IF NOT EXISTS user_access_keys
(
    file_id       text NOT NULL,
    sharee_id     text NOT NULL,
    encrypted_key text NOT NULL,
    CONSTRAINT pk_user_access_keys PRIMARY KEY (file_id, sharee_id),
    CONSTRAINT fk_user_access_keys_file_id_files_id FOREIGN KEY (file_id) REFERENCES files (id),
    CONSTRAINT fk_user_access_keys_sharee_id_accounts_name FOREIGN KEY (sharee_id) REFERENCES accounts (name)
);

CREATE TABLE IF NOT EXISTS usage_ledger
(
    file_id   text        NOT NULL,
    timestamp timestamptz NOT NULL,
    owner     text        NOT NULL,
    bytes     bigint      NOT NULL,
    CONSTRAINT pk_usage_ledger PRIMARY KEY (file_id, timestamp),
    CONSTRAINT fk_usage_ledger_file_id_files_id FOREIGN KEY (file_id) REFERENCES files (id),
    CONSTRAINT fk_usage_ledger_accounts_name FOREIGN KEY (owner) REFERENCES accounts (name)
);

CREATE INDEX IF NOT EXISTS usage_ledger_owner_index ON usage_ledger (owner);
