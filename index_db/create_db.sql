CREATE TABLE IF NOT EXISTS accounts (
	name		TEXT NOT NULL,
	public_key	TEXT NOT NULL,
	CONSTRAINT pk_accounts		PRIMARY KEY (name),
	CONSTRAINT uk_public_key	UNIQUE (public_key)
);

CREATE TABLE IF NOT EXISTS files (
	id					TEXT NOT NULL,
	parent				TEXT NOT NULL,
	parent_access_key   TEXT NOT NULL,
	is_folder			BOOLEAN NOT NULL,
	name				TEXT NOT NULL,
	owner				TEXT NOT NULL,
	signature			TEXT NOT NULL,
	deleted				BOOLEAN,
	metadata_version	BIGINT NOT NULL,
	content_version		BIGINT NOT NULL,
	CONSTRAINT pk_files						PRIMARY KEY (id),
	CONSTRAINT uk_files_name_parent			UNIQUE (parent, name),
	CONSTRAINT fk_files_parent_files_id	FOREIGN KEY (parent) REFERENCES files(id),
	CONSTRAINT fk_files_owner_accounts_name	FOREIGN KEY (owner) REFERENCES accounts(name)
);

CREATE TABLE IF NOT EXISTS user_access_keys (
	file_id			TEXT NOT NULL,
	sharee_id		TEXT NOT NULL,
	encrypted_key	TEXT NOT NULL,
	CONSTRAINT pk_user_access_keys							PRIMARY KEY (file_id, sharee_id),
	CONSTRAINT fk_user_access_keys_file_id_files_id			FOREIGN KEY (file_id) REFERENCES files(id),
	CONSTRAINT fk_user_access_keys_sharee_id_accounts_name	FOREIGN KEY (sharee_id) REFERENCES accounts(name)
);

CREATE TABLE IF NOT EXISTS usage_ledger (
	file_id			    TEXT NOT NULL,
	timestamp           timestamptz NOT NULL,
	owner			    TEXT NOT NULL,
    bytes               BIGINT NOT NULL,
	CONSTRAINT pk_usage_ledger                      PRIMARY KEY (file_id, timestamp),
	CONSTRAINT fk_usage_ledger_file_id_files_id     FOREIGN KEY (file_id) REFERENCES files(id),
	CONSTRAINT fk_usage_ledger_accounts_name        FOREIGN KEY (owner) REFERENCES accounts(name)
);

CREATE INDEX usage_ledger_owner_index ON usage_ledger(owner);