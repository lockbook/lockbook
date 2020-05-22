.PHONY: core
core: is_docker_running
	@docker build -f containers/Dockerfile.core . --tag core:$(hash) 

.PHONY: cargo_fmt
core_fmt:
	@echo The following files need formatting:
	@docker run core:$(hash) cargo +stable fmt -- --check -l

.PHONY: cargo_test
core_test:
	@docker run core:$(hash) cargo test --lib

.PHONY: cargo_push
core_push:
	@docker tag core:$(hash) docker.pkg.github.com/lockbook/lockbook/core:$(hash)
	@docker push docker.pkg.github.com/lockbook/lockbook/core:$(hash)

# Helpers
.PHONY: is_docker_running
is_docker_running:
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"
	
# For docker tags
hash := $(shell git rev-parse --short HEAD) 

