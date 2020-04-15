CREATE TABLE users (
	username	TEXT NOT NULL,
	public_key	TEXT NOT NULL,
	CONSTRAINT pk_users PRIMARY KEY (username)
);

CREATE TABLE files (
	file_id					TEXT NOT NULL,
	file_name				TEXT NOT NULL,
	file_path				TEXT NOT NULL,
	username				TEXT NOT NULL,
	file_content_version	BIGINT NOT NULL,
	file_metadata_version	BIGINT NOT NULL,
	deleted					BOOLEAN,
	CONSTRAINT pk_files PRIMARY KEY (file_id),
	CONSTRAINT unique_file_path UNIQUE (username, file_path),
	CONSTRAINT fk_files_username FOREIGN KEY (username) REFERENCES users(username)
);
