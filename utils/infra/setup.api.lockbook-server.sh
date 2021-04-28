#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q $TARGET exit || { echo "could not SSH into $TARGET"; exit 1; }

scp $2 $TARGET:/usr/bin/lockbook-server
scp $GIT_ROOT/server/instances/api/lockbook-server.service $TARGET:/etc/systemd/system/
scp $GIT_ROOT/server/instances/secret/prod-index-db.crt $TARGET:/root/
scp $GIT_ROOT/server/instances/secret/lockbook-server.prod $TARGET:/etc/default/lockbook-server

ssh $TARGET << EOF
  chmod +x /usr/bin/lockbook-server
  systemctl daemon-reload
  systemctl enable lockbook-server
  systemctl restart lockbook-server
  systemctl status lockbook-server
EOF