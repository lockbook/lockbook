CREATE TABLE accounts (
	name		TEXT NOT NULL,
	public_key	TEXT NOT NULL,
	CONSTRAINT pk_accounts		PRIMARY KEY (name),
	CONSTRAINT uk_public_key	UNIQUE (public_key)
);

CREATE TABLE folders (
	id					TEXT NOT NULL,
	parent_id			TEXT NOT NULL,
	name				TEXT NOT NULL,
	owner				TEXT NOT NULL,
	signature			TEXT NOT NULL,
	metadata_version	BIGINT NOT NULL,
	deleted				BOOLEAN,
	CONSTRAINT pk_folders						PRIMARY KEY (id),
	CONSTRAINT uk_folders_name_parent_id		UNIQUE (parent_id, name),
	CONSTRAINT fk_folders_parent_id_folders_id	FOREIGN KEY (parent_id) REFERENCES folders(id),
	CONSTRAINT fk_folders_owner_accounts_name	FOREIGN KEY (owner) REFERENCES accounts(name)
);

CREATE TABLE documents (
	id					TEXT NOT NULL,
	parent_id			TEXT NOT NULL,
	name				TEXT NOT NULL,
	owner				TEXT NOT NULL,
	signature			TEXT NOT NULL,
	metadata_version	BIGINT NOT NULL,
	content_version		BIGINT NOT NULL,
	deleted				BOOLEAN,
	CONSTRAINT pk_documents							PRIMARY KEY (id),
	CONSTRAINT uk_documents_name_parent_id			UNIQUE (parent_id, name),
	CONSTRAINT fk_documents_parent_id_folders_id	FOREIGN KEY (parent_id) REFERENCES folders(id),
	CONSTRAINT fk_documents_owner_accounts_name		FOREIGN KEY (owner) REFERENCES accounts(name)
);

CREATE TABLE folder_keys_for_accounts (
	file_id			TEXT NOT NULL,
	sharee_id		TEXT NOT NULL,
	encrypted_key	TEXT NOT NULL,
	CONSTRAINT pk_folder_keys_for_accounts							PRIMARY KEY (file_id, sharee_id),
	CONSTRAINT fk_folder_keys_for_accounts_file_id_folders_id		FOREIGN KEY (file_id) REFERENCES folders(id),
	CONSTRAINT fk_folder_keys_for_accounts_sharee_id_accounts_name	FOREIGN KEY (sharee_id) REFERENCES accounts(name)
);

CREATE TABLE folder_keys_for_folders (
	file_id			TEXT NOT NULL,
	sharee_id		TEXT NOT NULL,
	encrypted_key	TEXT NOT NULL,
	CONSTRAINT pk_folder_keys_for_folders							PRIMARY KEY (file_id, sharee_id),
	CONSTRAINT fk_folder_keys_for_folders_file_id_folders_id		FOREIGN KEY (file_id) REFERENCES folders(id),
	CONSTRAINT fk_folder_keys_for_folders_sharee_id_folders_id		FOREIGN KEY (sharee_id) REFERENCES folders(id)
);

CREATE TABLE document_keys_for_accounts (
	file_id			TEXT NOT NULL,
	sharee_id		TEXT NOT NULL,
	encrypted_key	TEXT NOT NULL,
	CONSTRAINT pk_document_keys_for_accounts							PRIMARY KEY (file_id, sharee_id),
	CONSTRAINT fk_document_keys_for_accounts_file_id_folders_id			FOREIGN KEY (file_id) REFERENCES documents(id),
	CONSTRAINT fk_document_keys_for_accounts_sharee_id_accounts_name	FOREIGN KEY (sharee_id) REFERENCES accounts(name)
);

CREATE TABLE document_keys_for_folders (
	file_id			TEXT NOT NULL,
	sharee_id		TEXT NOT NULL,
	encrypted_key	TEXT NOT NULL,
	CONSTRAINT pk_document_keys_for_folders							PRIMARY KEY (file_id, sharee_id),
	CONSTRAINT fk_document_keys_for_folders_file_id_folders_id		FOREIGN KEY (file_id) REFERENCES documents(id),
	CONSTRAINT fk_document_keys_for_folders_sharee_id_folders_id	FOREIGN KEY (sharee_id) REFERENCES folders(id)
);

CREATE VIEW file_keys_for_sharees AS
SELECT * FROM folder_keys_for_accounts UNION
SELECT * FROM folder_keys_for_folders UNION
SELECT * FROM document_keys_for_accounts UNION
SELECT * FROM document_keys_for_folders;
