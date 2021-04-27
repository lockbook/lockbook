#!/bin/sh

(
	cd ../../core
	cargo fmt
)

(
	cd ../../server/server
	cargo fmt
)

(
	cd ../../clients/cli
	cargo fmt
)

(
	cd ../../clients/linux
	cargo fmt
)

(
	cd ../../server/admin
	cargo fmt
)
