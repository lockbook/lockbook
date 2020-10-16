.PHONY: all
all: core_fmt core_test core_lint server_fmt server_lint server_test cli_fmt cli_lint cli_test integration_tests_run kotlin_interface_tests_run android
	echo "Done!"

.PHONY: clean
clean:
	-docker network prune -f

.PHONY: exorcise
exorcise:
	-docker rm -f $$(docker ps -a -q)
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

.PHONY: server_tests
server_tests: is_docker_running
	docker build -f containers/Dockerfile.server . --tag server_tests:$(hash)

.PHONY: server_tests_run
server_tests_run: server server_tests
	HASH=$(hash) docker-compose -f containers/docker-compose-server-tests.yml --project-name=server-tests-$(hash) down
	HASH=$(hash) docker-compose -f containers/docker-compose-server-tests.yml --project-name=server-tests-$(hash) up --exit-code-from=server_tests

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
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=integration-tests-$(hash) down
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=integration-tests-$(hash) up --exit-code-from=integration_tests

.PHONY: android
android: is_docker_running
	docker build -f containers/Dockerfile.android . --tag android:$(hash)

.PHONY: android_lint
android_lint: android
	docker run android:$(hash) ./gradlew lint

.PHONY: android_fmt
android_fmt: android
	docker run android:$(hash) ./gradlew lintKotlin 

.PHONY: kotlin_interface_tests
kotlin_interface_tests: is_docker_running
	docker build -f containers/Dockerfile.kotlin_interface_tests . --tag kotlin_interface_tests:$(hash)

.PHONY: kotlin_interface_tests_run
kotlin_interface_tests_run: server kotlin_interface_tests
	HASH=$(hash) docker-compose -f containers/docker-compose-kotlin-interface-tests.yml --project-name=kotlin-$(hash) down
	HASH=$(hash) docker-compose -f containers/docker-compose-kotlin-interface-tests.yml --project-name=kotlin-$(hash) up --exit-code-from=kotlin_interface_tests

.PHONY: swift_interface_tests
swift_interface_tests: is_docker_running
	docker build -f containers/Dockerfile.swift_interface_tests . --tag swift_interface_tests:$(hash)

.PHONY: swift_interface_tests
swift_interface_tests_run: server swift_interface_tests_run
	HASH=$(hash) docker-compose -f containers/docker-compose-swift-interface-tests.yml --project-name=swift-$(hash) down
	HASH=$(hash) docker-compose -f containers/docker-compose-swift-interface-tests.yml --project-name=swift-$(hash) up --exit-code-from=swift_interface_tests

.PHONY: performance
performance: is_docker_running
	docker build -f containers/Dockerfile.performance . --tag performance:$(hash)

.PHONY: performance_bench
performance_bench: performance server
	HASH=$(hash) TYPE="performance" docker-compose -f containers/common-services.yml -f containers/docker-compose-performance.yml --project-name=performance-$(hash) down
	HASH=$(hash) TYPE="performance" docker-compose -f containers/common-services.yml -f containers/docker-compose-performance.yml --project-name=performance-$(hash) up --exit-code-from=performance_bench

.PHONY: performance_bench_report
performance_bench_report: is_docker_running
	docker container cp "$$(docker inspect --format="{{.Id}}" performance-performance-$(hash))":/core/simple-create_write_read.svg simple-create_write_read.svg

# Helpers
.PHONY: is_docker_running
is_docker_running:
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

# For docker tags
hash := $(shell git rev-parse --short HEAD)
branch := $(if ${BRANCH},${BRANCH},$(shell git rev-parse --abbrev-ref HEAD))
