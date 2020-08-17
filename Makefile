.PHONY: all
all: core_fmt core_test core_lint server_fmt server_lint server_test cli_fmt cli_lint cli_test integration_tests_fmt integration_tests_lint integration_tests_run android
	echo "Done!"

.PHONY: clean
clean:
	-docker rm -f $$(docker ps -a -q)
	-docker rmi -f $$(docker images -q)
	-docker system prune -a -f
	-git clean -fdX

.PHONY: core
core: is_docker_running
	docker build -f containers/Dockerfile.core . --tag core:$(hash)

.PHONY: core_fmt
core_fmt: core
	@echo The following files need formatting:
	docker run core:$(hash) cargo +stable fmt -- --check -l

.PHONY: core_lint
core_lint: core
	docker run core:$(hash) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::missing-safety-doc -A clippy::expect-fun-call -A clippy::too-many-arguments

.PHONY: core_test
core_test: core
	docker run core:$(hash) cargo test --lib

.PHONY: server
server: is_docker_running
	docker build -f containers/Dockerfile.server . --tag server:$(hash)

.PHONY: server_fmt
server_fmt: server
	@echo The following files need formatting:
	docker run server:$(hash) cargo +stable fmt -- --check -l

.PHONY: server_lint
server_lint: server
	docker run server:$(hash) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call -A clippy::too-many-arguments

.PHONY: server_test
server_test: server
	docker run server:$(hash) cargo test

.PHONY: cli
cli: is_docker_running
	docker build -f containers/Dockerfile.cli . --tag cli:$(hash)

.PHONY: cli_fmt
cli_fmt: cli
	@echo The following files need formatting:
	docker run cli:$(hash) cargo +stable fmt -- --check -l

.PHONY: cli_lint
cli_lint: cli
	docker run cli:$(hash) cargo +stable clippy -- -D warnings -A clippy::redundant-field-names -A clippy::ptr-arg -A clippy::missing-safety-doc -A clippy::expect-fun-call -A clippy::too-many-arguments

.PHONY: cli_test
cli_test: cli
	docker run cli:$(hash) cargo test

.PHONY: integration_tests
integration_tests: is_docker_running
	docker build -f containers/Dockerfile.integration_tests . --tag integration_tests:$(hash)

.PHONY: integration_tests_run
integration_tests_run: integration_tests server
	HASH=$(hash) docker-compose down
	HASH=$(hash) docker-compose up --exit-code-from=integration_tests

.PHONY: android
android:
	docker build -f containers/Dockerfile.android . --tag android:$(hash)

.PHONY: android_lint
android:
	docker run cli:$(hash) ./gradlew lint

# Helpers
.PHONY: is_docker_running
is_docker_running:
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

# For docker tags
hash := $(shell git rev-parse --short HEAD)
branch := $(if ${BRANCH},${BRANCH},$(shell git rev-parse --abbrev-ref HEAD))
