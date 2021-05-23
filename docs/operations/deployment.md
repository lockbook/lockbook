# Deployment rituals

## What changed? 

#### Schema

`./upgrade_qa_schema.sh` in QA then in Prod

#### Server

`./run_server_with_qa_dbs.sh` in QA then in Prod

#### Core

1. `./bump_aur_version.sh`
2. `./release_linux_cli.sh`
3. `./release_macos_cli_bump_brew.sh`