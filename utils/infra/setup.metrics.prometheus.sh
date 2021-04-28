#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q $TARGET exit || { echo "could not SSH into $TARGET"; exit 1; }

if [ "$2" == "-f" ]; then
TMP=$(date +%s)
ssh $TARGET << EOF
  mkdir -p /tmp/$TMP
  cd /tmp/$TMP
  mkdir -p /etc/prometheus
  curl -LO https://github.com/prometheus/prometheus/releases/download/v2.26.0/prometheus-2.26.0.linux-amd64.tar.gz
  tar -xvf prometheus-2.26.0.linux-amd64.tar.gz
  ls
  mv prometheus-2.26.0.linux-amd64/prometheus /usr/bin/
  mv prometheus-2.26.0.linux-amd64/promtool /usr/bin/
  rm -r /tmp/$TMP
EOF
fi

scp $GIT_ROOT/server/instances/prometheus/prometheus.yml $TARGET:/etc/prometheus/
scp $GIT_ROOT/server/instances/prometheus/prometheus.service $TARGET:/etc/systemd/system/

ssh $TARGET << EOF
  systemctl daemon-reload
  systemctl enable prometheus
  systemctl restart prometheus
  systemctl status prometheus
EOF