#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q $TARGET exit || { echo "could not SSH into $TARGET"; exit 1; }

TMP=$(date +%s)
ssh $TARGET << EOF
  mkdir -p /tmp/$TMP
  cd /tmp/$TMP
  curl -LO https://github.com/prometheus/node_exporter/releases/download/v1.1.2/node_exporter-1.1.2.linux-amd64.tar.gz
  tar -xvf node_exporter-1.1.2.linux-amd64.tar.gz
  ls
  mv node_exporter-1.1.2.linux-amd64/node_exporter /usr/bin/
  rm -r /tmp/$TMP
EOF

scp $GIT_ROOT/server/instances/prometheus/node-exporter.service $TARGET:/etc/systemd/system/

ssh $TARGET << EOF
  systemctl daemon-reload
  systemctl enable node-exporter
  systemctl start node-exporter
  systemctl status node-exporter
EOF