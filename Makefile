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

.PHONY: core_test
core_test:
	docker run core:$(branch) cargo test --lib

.PHONY: core_push
core_push:
	docker tag core:$(branch) docker.pkg.github.com/lockbook/lockbook/core:$(branch)
	docker push docker.pkg.github.com/lockbook/lockbook/core:$(branch)

# Helpers
.PHONY: is_docker_running
is_docker_running: 
	@echo "Checking if docker is running"
	@docker ps -q
	@echo "Docker is running"

# For docker tags
hash := $(shell git rev-parse --short HEAD) 
branch := $(if ${BRANCH},${BRANCH},$(shell git rev-parse --abbrev-ref HEAD))

# Github actions doesn't support layers, so we use --cache-from and try to grab the
# closest image we can (this branch, otherwise master, otherwise nothing)
# When you do this every docker build is rebuilt from this cache point. Maybe buildkit
# will improve this situation, at the moment I do not have the desire to look into it.
# In an ideal case core_fmt depends on core so you just have oneliners. However github
# actions will rebuild core each time, which takes about 1m. As the purpose of this
# Makefile is primarily portable automataed build instructions && debugging when there
# are build failures, this dependency is not expressed and the user unfortunately has to
# make core && make core_test to replicate issues locally.
