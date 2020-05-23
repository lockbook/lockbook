.PHONY: core
core: is_docker_running core_pull
	docker build --cache-from docker.pkg.github.com/lockbook/lockbook/core:$(branch) -f containers/Dockerfile.core . --tag core:$(branch) 

.PHONY: core_pull
core_pull:
	docker pull docker.pkg.github.com/lockbook/lockbook/core:$(branch)

.PHONY: cargo_fmt
core_fmt: core
	@echo The following files need formatting:
	docker run core:$(branch) cargo +stable fmt -- --check -l

.PHONY: cargo_test
core_test: core
	docker run core:$(branch) cargo test --lib

.PHONY: cargo_push
core_push: core
	docker tag core:$(branch) docker.pkg.github.com/lockbook/lockbook/core:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/core:$(branch)

# Helpers
.PHONY: is_docker_running
is_docker_running: 
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

test:
	echo $(branch)
	
# For docker tags
hash := $(shell git rev-parse --short HEAD) 
branch := $(shell ./containers/get_branch.sh)
