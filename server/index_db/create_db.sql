CREATE TABLE IF NOT EXISTS stripe_payees
(
    id BIGSERIAL NOT NULL PRIMARY KEY
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
    id           BIGSERIAL,
    bytes_cap    BIGINT NOT NULL,
    valid_until  TIMESTAMPTZ,
    payee_stripe BIGINT,
    payee_apple  BIGINT,
    payee_google BIGINT,
    CONSTRAINT pk_account_tiers PRIMARY KEY (id),
    CONSTRAINT fk_account_tiers_payee_stripe_stripe_payees_id FOREIGN KEY (payee_stripe) REFERENCES stripe_payees (id),
    CONSTRAINT fk_account_tiers_payee_apple_apple_payees_id FOREIGN KEY (payee_apple) REFERENCES apple_payees (id),
    CONSTRAINT fk_account_tiers_payee_google_google_payees_id FOREIGN KEY (payee_google) REFERENCES google_payees (id),
    CONSTRAINT at_most_one_payment_method CHECK (
        NOT (
                (payee_stripe IS NOT NULL AND payee_apple IS NOT NULL) AND
                (payee_stripe IS NOT NULL AND payee_google IS NOT NULL) AND
                (payee_apple IS NOT NULL AND payee_google IS NOT NULL) AND
                (payee_stripe IS NOT NULL AND payee_apple IS NOT NULL AND payee_google IS NOT NULL
            )
        )
    )
);

CREATE TABLE IF NOT EXISTS accounts
(
    name         TEXT   NOT NULL,
    public_key   TEXT   NOT NULL,
    account_tier BIGINT NOT NULL,
    CONSTRAINT pk_accounts PRIMARY KEY (name),
    CONSTRAINT fk_accounts_account_tier_account_tiers_id FOREIGN KEY (account_tier) REFERENCES account_tiers (id),
    CONSTRAINT uk_public_key UNIQUE (public_key)
);

CREATE TABLE IF NOT EXISTS files
(
    id                TEXT    NOT NULL,
    parent            TEXT    NOT NULL,
    parent_access_key TEXT    NOT NULL,
    is_folder         BOOLEAN NOT NULL,
    name              TEXT    NOT NULL,
    owner             TEXT    NOT NULL,
    signature         TEXT    NOT NULL,
    deleted           BOOLEAN NOT NULL,
    metadata_version  BIGINT  NOT NULL,
    content_version   BIGINT  NOT NULL,
    document_size     BIGINT,
    CONSTRAINT pk_files PRIMARY KEY (id),
    CONSTRAINT fk_files_parent_files_id FOREIGN KEY (parent) REFERENCES files (id),
    CONSTRAINT fk_files_owner_accounts_name FOREIGN KEY (owner) REFERENCES accounts (name),
    CONSTRAINT documents_must_have_size CHECK (
        is_folder OR document_size IS NOT NULL
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_files_name_parent ON files (parent, name) WHERE (NOT deleted);

CREATE TABLE IF NOT EXISTS user_access_keys
(
    file_id       TEXT NOT NULL,
    sharee_id     TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    CONSTRAINT pk_user_access_keys PRIMARY KEY (file_id, sharee_id),
    CONSTRAINT fk_user_access_keys_file_id_files_id FOREIGN KEY (file_id) REFERENCES files (id),
    CONSTRAINT fk_user_access_keys_sharee_id_accounts_name FOREIGN KEY (sharee_id) REFERENCES accounts (name)
);
