#!/bin/bash
if [[ -z "${GITHUB_REF}" ]]; then
	git rev-parse --abbrev-ref HEAD
else
	echo ${GITHUB_REF##*/}
fi
