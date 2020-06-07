CREATE TABLE users (
	name		TEXT NOT NULL,
	public_key	TEXT NOT NULL,
	CONSTRAINT pk_users			PRIMARY KEY (name),
	CONSTRAINT uk_public_key	UNIQUE (public_key)
);

CREATE TABLE folders (
	id					TEXT NOT NULL,
	parent_id			TEXT NOT NULL,
	name				TEXT NOT NULL,
	owner				TEXT NOT NULL,
	metadata_version	BIGINT NOT NULL,
	deleted				BOOLEAN,
	CONSTRAINT pk_folders						PRIMARY KEY (id),
	CONSTRAINT uk_folders_name_parent_id		UNIQUE (parent_id, name),
	CONSTRAINT fk_folders_parent_id_folders_id	FOREIGN KEY (parent_id) REFERENCES folders(id),
	CONSTRAINT fk_folders_owner_users_name		FOREIGN KEY (owner) REFERENCES users(name)
);

CREATE TABLE documents (
	id					TEXT NOT NULL,
	parent_id			TEXT NOT NULL,
	name				TEXT NOT NULL,
	owner				TEXT NOT NULL,
	metadata_version	BIGINT NOT NULL,
	content_version		BIGINT NOT NULL,
	deleted				BOOLEAN,
	CONSTRAINT pk_documents							PRIMARY KEY (id),
	CONSTRAINT uk_documents_name_parent_id			UNIQUE (parent_id, name),
	CONSTRAINT fk_documents_parent_id_folders_id	FOREIGN KEY (parent_id) REFERENCES folders(id),
	CONSTRAINT fk_documents_owner_users_name		FOREIGN KEY (owner) REFERENCES users(name)
);
