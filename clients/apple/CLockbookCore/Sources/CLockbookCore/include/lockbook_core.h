#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

const char *calculate_work(const char *writeable_path);

const char *create_account(const char *writeable_path, const char *username);

const char *create_file(const char *writeable_path,
                        const char *name,
                        const char *parent,
                        const char *file_type);

const char *create_file_at_path(const char *writeable_path, const char *path_and_name);

const char *execute_work(const char *writeable_path, const char *work_unit);

const char *export_account(const char *writeable_path);

const char *get_account(const char *writeable_path);

const char *get_api_loc(void);

const char *get_file_by_path(const char *writeable_path, const char *path);

const char *get_last_synced(const char *writeable_path);

const char *get_root(const char *writeable_path);

const char *import_account(const char *writeable_path, const char *account_string);

void init_logger_safely(const char *writeable_path);

const char *list_metadatas(const char *writeable_path);

const char *list_paths(const char *writeable_path, const char *filter);

const char *move_file(const char *writeable_path, const char *id, const char *new_parent);

const char *read_document(const char *writeable_path, const char *id);

void release_pointer(char *s);

const char *rename_file(const char *writeable_path, const char *id, const char *new_name);

const char *set_last_synced(const char *writeable_path, uint64_t last_sync);

const char *sync_all(const char *writeable_path);

const char *write_document(const char *writeable_path, const char *id, const char *content);
