# TL;DR: run make x before running make x_fmt, make x_lint, or make x_test
# Github actions doesn't support layers, so we use --cache-from and try to grab the
# closest image we can (this branch, otherwise master, otherwise nothing)
# When you do this every docker build is rebuilt from this cache point. Maybe buildkit
# will improve this situation, at the moment I do not have the desire to look into it.
# In an ideal case core_fmt depends on core so you just have oneliners. However github
# actions will rebuild core each time, which takes about 1m. As the purpose of this
# Makefile is primarily portable automated build instructions && debugging when there
# are build failures, this dependency is not expressed and the user unfortunately has to
# make core && make core_test to replicate issues locally.

.PHONY: core_cached
core_cached: is_docker_running core_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/core:$(branch) -f containers/Dockerfile.core . --tag core:$(branch) 

.PHONY: core_pull
core_pull:
	docker pull docker.pkg.github.com/lockbook/lockbook/core:$(branch) || docker pull docker.pkg.github.com/lockbook/lockbook/core:master || echo "Failed to pull, ERROR IGNORED"

.PHONY: core
core:
	docker build -f containers/Dockerfile.core . --tag core:$(branch)

.PHONY: core_fmt
core_fmt:
	@echo The following files need formatting:
	docker run core:$(branch) cargo +stable fmt -- --check -l

.PHONY: core_lint
core_lint:
	docker run core:$(branch) cargo clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: core_test
core_test:
	docker run core:$(branch) cargo test --lib

.PHONY: core_push
core_push:
	docker tag core:$(branch) docker.pkg.github.com/lockbook/lockbook/core:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/core:$(branch)

.PHONY: server_cached
server_cached: is_docker_running server_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/server:$(branch) -f containers/Dockerfile.server . --tag server:$(branch) 

.PHONY: server_pull
server_pull:
	docker pull docker.pkg.github.com/lockbook/lockbook/server:$(branch) || docker pull docker.pkg.github.com/lockbook/lockbook/server:master || echo "Failed to pull, ERROR IGNORED"

.PHONY: server
server:
	docker build -f containers/Dockerfile.server . --tag server:$(branch)

.PHONY: server_fmt
server_fmt:
	@echo The following files need formatting:
	docker run server:$(branch) cargo +stable fmt -- --check -l

.PHONY: server_lint
server_lint:
	docker run server:$(branch) cargo clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: server_test
server_test:
	docker run server:$(branch) cargo test

.PHONY: server_push
server_push:
	docker tag server:$(branch) docker.pkg.github.com/lockbook/lockbook/server:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/server:$(branch)

.PHONY: cli_cached
cli_cached: is_docker_running cli_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/cli:$(branch) -f containers/Dockerfile.cli . --tag cli:$(branch) 

.PHONY: cli_pull
cli_pull:
	docker pull docker.pkg.github.com/lockbook/lockbook/cli:$(branch) || docker pull docker.pkg.github.com/lockbook/lockbook/cli:master || echo "Failed to pull, ERROR IGNORED"

.PHONY: cli
cli:
	docker build -f containers/Dockerfile.cli . --tag cli:$(branch)

.PHONY: cli_fmt
cli_fmt:
	@echo The following files need formatting:
	docker run cli:$(branch) cargo +stable fmt -- --check -l

.PHONY: cli_lint
cli_lint:
	docker run cli:$(branch) cargo clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: cli_test
cli_test:
	docker run cli:$(branch) cargo test

.PHONY: cli_push
cli_push:
	docker tag cli:$(branch) docker.pkg.github.com/lockbook/lockbook/cli:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/cli:$(branch)

.PHONY: integration_tests_cached
integration_tests_cached: is_docker_running integration_tests_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch) -f containers/Dockerfile.integration_tests . --tag integration_tests:$(branch) 

.PHONY: integration_tests_pull
integration_tests_pull:
	docker pull docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch) || docker pull docker.pkg.github.com/lockbook/lockbook/test:master || echo "Failed to pull, ERROR IGNORED"

.PHONY: integration_tests
integration_tests:
	docker build -f containers/Dockerfile.integration_tests . --tag integration_tests:$(branch)

.PHONY: integration_tests_fmt
integration_tests_fmt:
	@echo The following files need formatting:
	docker run integration_tests:$(branch) cargo +stable fmt -- --check -l

.PHONY: integration_tests_lint
integration_tests_lint:
	docker run integration_tests:$(branch) cargo clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: integration_tests_run
integration_tests_run:
	# Remove containers in case they weren't cleaned up last time
	-docker rm --force test
	-docker rm --force lockbook
	-docker rm --force indexdbconfig
	-docker rm --force indexdb
	-docker rm --force filesdbconfig
	-docker rm --force filesdb
	# Start Minio
	(set -a && . containers/test.env && docker run -dP --name=filesdb --net=host -e MINIO_REGION_NAME=$$FILES_DB_REGION minio/minio:RELEASE.2020-05-16T01-33-21Z server /data)
	# Configure Minio
	docker run --name=filesdbconfig --net=host --env-file=containers/test.env --entrypoint=sh minio/mc:RELEASE.2020-05-16T01-44-37Z -c '\
		while ! nc -z $$FILES_DB_HOST $$FILES_DB_PORT; do echo "Waiting for Minio to start..." && sleep 0.2; done; \
		mc config host add filesdb $$FILES_DB_SCHEME://$$FILES_DB_HOST:$$FILES_DB_PORT $$FILES_DB_ACCESS_KEY $$FILES_DB_SECRET_KEY && \
		mc mb --region=$$FILES_DB_REGION filesdb/$$FILES_DB_BUCKET && \
		mc policy set public filesdb/testbucket \
	'
	# Start Postgres
	docker run -dP --name=indexdb --net=host -e POSTGRES_HOST_AUTH_METHOD=trust postgres:12.3
	# Configure Postgres
	docker run --name=indexdbconfig --net=host --env-file=containers/test.env --entrypoint=sh -v `pwd`/index_db:/index_db postgres:12.3 -c '\
		while ! pg_isready -h $$INDEX_DB_HOST -p $$INDEX_DB_PORT -U $$INDEX_DB_USER; do echo "Waiting for Postgres to start..." && sleep 0.2; done; \
		psql -wq -h $$INDEX_DB_HOST -p $$INDEX_DB_PORT -U $$INDEX_DB_USER --db $$INDEX_DB_DB -f /index_db/create_db.sql \
	'
	# Start Lockbook Server
	docker run -dP --name=lockbook --net=host --env-file=containers/test.env server:$(branch) cargo run
	# Run tests
	docker run --name=test --net=host --env-file=containers/test.env integration_tests:$(branch) cargo test
	# Remove containers
	-docker rm --force test
	-docker rm --force lockbook
	-docker rm --force indexdbconfig
	-docker rm --force indexdb
	-docker rm --force filesdbconfig
	-docker rm --force filesdb

.PHONY: integration_tests_push
integration_tests_push:
	docker tag integration_tests:$(branch) docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch)

# Helpers
.PHONY: is_docker_running
is_docker_running: 
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

# For docker tags
hash := $(shell git rev-parse --short HEAD) 
branch := $(if ${BRANCH},${BRANCH},$(shell git rev-parse --abbrev-ref HEAD))
