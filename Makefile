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

.PHONY: all
all: core server cli integration_tests
	$(MAKE) core_fmt
	$(MAKE) core_lint
	$(MAKE) core_test

	$(MAKE) server_fmt
	$(MAKE) server_lint
	$(MAKE) server_test

	$(MAKE) cli_fmt
	$(MAKE) cli_lint
	$(MAKE) cli_test

	$(MAKE) integration_tests_fmt
	$(MAKE) integration_tests_lint
	$(MAKE) integration_tests_run

	-$(MAKE) core_push
	-$(MAKE) server_push
	-$(MAKE) cli_push
	-$(MAKE) integration_tests_push
.PHONY: clean
clean:
	-docker rm -f $$(docker ps -a -q)
	-docker rmi -f $$(docker images -q)

.PHONY: core_pull
core_pull:
	-docker pull docker.pkg.github.com/lockbook/lockbook/core:$(branch)

.PHONY: core_cached
core_cached: core_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/core:$(branch) -f containers/Dockerfile.core . --tag core:$(branch)

.PHONY: core
core: is_docker_running
	docker build -f containers/Dockerfile.core . --tag core:$(branch)

.PHONY: core_fmt
core_fmt:
	@echo The following files need formatting:
	docker run core:$(branch) cargo +stable fmt -- --check -l

.PHONY: core_lint
core_lint:
	docker run core:$(branch) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: core_test
core_test:
	docker run core:$(branch) cargo test --lib

.PHONY: core_push
core_push:
	docker tag core:$(branch) docker.pkg.github.com/lockbook/lockbook/core:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/core:$(branch)

.PHONY: server_pull
server_pull:
	-docker pull docker.pkg.github.com/lockbook/lockbook/server:$(branch)

.PHONY: server_cached
server_cached: server_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/server:$(branch) -f containers/Dockerfile.server . --tag server:$(branch)

.PHONY: server
server: is_docker_running
	docker build -f containers/Dockerfile.server . --tag server:$(branch)

.PHONY: server_fmt
server_fmt:
	@echo The following files need formatting:
	docker run server:$(branch) cargo +stable fmt -- --check -l

.PHONY: server_lint
server_lint:
	docker run server:$(branch) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: server_test
server_test:
	docker run server:$(branch) cargo test

.PHONY: server_push
server_push:
	docker tag server:$(branch) docker.pkg.github.com/lockbook/lockbook/server:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/server:$(branch)

.PHONY: cli_pull
cli_pull:
	-docker pull docker.pkg.github.com/lockbook/lockbook/cli:$(branch)

.PHONY: cli_cached
cli_cached: cli_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/cli:$(branch) -f containers/Dockerfile.cli . --tag cli:$(branch)

.PHONY: cli
cli: is_docker_running
	docker build -f containers/Dockerfile.cli . --tag cli:$(branch)

.PHONY: cli_fmt
cli_fmt:
	@echo The following files need formatting:
	docker run cli:$(branch) cargo +stable fmt -- --check -l

.PHONY: cli_lint
cli_lint:
	docker run cli:$(branch) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: cli_test
cli_test:
	docker run cli:$(branch) cargo test

.PHONY: cli_push
cli_push:
	docker tag cli:$(branch) docker.pkg.github.com/lockbook/lockbook/cli:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/cli:$(branch)

.PHONY: integration_tests_pull
integration_tests_pull:
	-docker pull docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch)

.PHONY: integration_tests_cached
integration_tests_cached: integration_tests_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/integration_tests:$(branch) -f containers/Dockerfile.integration_tests . --tag integration_tests:$(branch)

.PHONY: integration_tests
integration_tests: is_docker_running
	docker build -f containers/Dockerfile.integration_tests . --tag integration_tests:$(branch)

.PHONY: integration_tests_fmt
integration_tests_fmt:
	@echo The following files need formatting:
	docker run integration_tests:$(branch) cargo +stable fmt -- --check -l

.PHONY: integration_tests_lint
integration_tests_lint:
	docker run integration_tests:$(branch) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call

.PHONY: integration_tests_run
integration_tests_run:
	BRANCH=$(branch) docker-compose down
	BRANCH=$(branch) docker-compose up --exit-code-from=integration_tests

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
