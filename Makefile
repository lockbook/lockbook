.PHONY: all
all: core_fmt core_test core_lint server_fmt server_lint server_tests cli_fmt cli_lint cli_test integration_tests_run linux_test swift_interface_tests_run kotlin_interface_tests_run android
	echo "Done!"

.PHONY: clean
clean:
	-docker network prune -f
	-docker image prune -af --filter "until=24h"
	-docker container prune
	-docker volume prune

.PHONY: exorcise
exorcise:
	-docker rm -f $$(docker ps -a -q)
	-docker system prune -a -f
	-git clean -fdX

.PHONY: core
core: is_docker_running
	docker build --target core-build -f containers/Dockerfile.core . --tag core:$(hash)

.PHONY: core_fmt
core_fmt: core
	@echo The following files need formatting:
	docker build --target core-fmt -f containers/Dockerfile.core . --tag core_fmt:$(hash)

.PHONY: core_lint
core_lint: core
	docker build --target core-lint -f containers/Dockerfile.core . --tag core_lint:$(hash)

.PHONY: core_test
core_test: core
	docker build --target core-test -f containers/Dockerfile.core . --tag core_test:$(hash)

.PHONY: server
server: is_docker_running
	docker build --target server-build -f containers/Dockerfile.server . --tag server:$(hash)

.PHONY: server_fmt
server_fmt: server
	@echo The following files need formatting:
	docker build --target server-fmt -f containers/Dockerfile.server . --tag server_fmt:$(hash)

.PHONY: server_lint
server_lint: server
	docker build --target server-lint -f containers/Dockerfile.server . --tag server_lint:$(hash)

.PHONY: server_tests
server_tests: is_docker_running
	docker build --target server-build -f containers/Dockerfile.server . --tag server_tests:$(hash)

.PHONY: server_tests_run
server_tests_run: server server_tests db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate server_tests
	exit $$(docker wait server_tests-client-$(hash))

.PHONY: cli
cli: is_docker_running
	docker build --target cli-build -f containers/Dockerfile.cli . --tag cli:$(hash)

.PHONY: cli_fmt
cli_fmt: cli
	@echo The following files need formatting:compute_usage
	docker build --target cli-fmt -f containers/Dockerfile.cli . --tag cli_fmt:$(hash)

.PHONY: cli_lint
cli_lint: cli
	docker build --target cli-lint -f containers/Dockerfile.cli . --tag cli_lint:$(hash)

.PHONY: cli_test
cli_test: cli
	docker build --target cli-test -f containers/Dockerfile.cli . --tag cli_test:$(hash)

.PHONY: linux
linux: is_docker_running
	docker build -f containers/Dockerfile.linux . --tag linux:$(hash) --tag linux:$(hash)

.PHONY: linux_fmt
linux_fmt: linux
	@echo The following files need formatting:
	docker build --target linux-build -f containers/Dockerfile.linux . --tag linux_fmt:$(hash)

.PHONY: linux_lint
linux_lint: linux
	docker build --target linux-lint -f containers/Dockerfile.linux . --tag linux_lint:$(hash)

.PHONY: linux_test
linux_test: linux
	docker build --target linux-test -f containers/Dockerfile.linux . --tag linux_test:$(hash)

.PHONY: integration_tests
integration_tests: is_docker_running
	docker build --target integration-tests -f containers/Dockerfile.core . --tag integration_tests:$(hash)

.PHONY: integration_tests_run
integration_tests_run: integration_tests server db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up integration_tests
	exit $$(docker wait integration_tests-integration-$(hash))

.PHONY: android
android: is_docker_running
	docker build --target android-build -f containers/Dockerfile.android . --tag android:$(hash)

.PHONY: android_lint
android_lint: android
	docker build --target android-lint -f containers/Dockerfile.android . --tag android_lint:$(hash)

.PHONY: android_fmt
android_fmt: android
	docker build --target android-fmt -f containers/Dockerfile.android . --tag android_fmt:$(hash)

.PHONY: kotlin_interface_tests
kotlin_interface_tests: is_docker_running
	docker build --target kotlin-interface-tests -f containers/Dockerfile.android . --tag kotlin_interface_tests:$(hash)

.PHONY: kotlin_interface_tests_run
kotlin_interface_tests_run: server kotlin_interface_tests db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate kotlin_interface_tests
	exit $$(docker wait kotlin_interface_tests-kotlin-$(hash))

.PHONY: swift_interface_tests
swift_interface_tests: is_docker_running
	docker build -f containers/Dockerfile.swift_interface_tests . --tag swift_interface_tests:$(hash)

.PHONY: swift_interface_tests_run
swift_interface_tests_run: server swift_interface_tests db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate swift_interface_tests
	exit $$(docker wait swift_interface_tests-swift-$(hash))

.PHONY: csharp_interface_tests
csharp_interface_tests: is_docker_running
	docker build -f containers/Dockerfile.csharp_interface_tests . --tag csharp_interface_tests:$(hash)

.PHONY: csharp_interface_tests_run
csharp_interface_tests_run: server csharp_interface_tests db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate csharp_interface_tests
	exit $$(docker wait csharp_interface_tests-csharp-$(hash))

.PHONY: performance
performance: is_docker_running
	docker build -f containers/Dockerfile.performance . --tag performance:$(hash)

.PHONY: performance_bench
performance_bench: performance server db_container
	HASH=$(hash) TYPE="performance" docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate performance_bench
	exit $$(docker wait performance-performance-$(hash))

.PHONY: performance_bench_report
performance_bench_report: is_docker_running
	docker container cp "$$(docker inspect --format="{{.Id}}" performance-performance-$(hash))":/core/simple-create_write_read.svg .

.PHONY: db_container
db_container: is_docker_running
	HASH=$(hash) docker build -f containers/Dockerfile.db . --tag db_with_migration-$(hash)

.PHONY: dev_stack_run
dev_stack_run: server db_container
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) up --no-recreate lockbook_server

.PHONY: kill_dev_stack
kill_dev_stack:
	HASH=$(hash) docker-compose -f containers/docker-compose-integration-tests.yml --project-name=lockbook-$(hash) down

# Helpers
.PHONY: is_docker_running
is_docker_running:
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

# For docker tags
hash := $(shell git rev-parse --short HEAD)
branch := $(if ${BRANCH},${BRANCH},$(shell git rev-parse --abbrev-ref HEAD))
