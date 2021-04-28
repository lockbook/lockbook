#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q $TARGET exit || { echo "could not SSH into $TARGET"; exit 1; }

TMP=$(date +%s)
# ssh $TARGET << EOF
#   mkdir -p /tmp/$TMP
#   cd /tmp/$TMP
#   mkdir -p /etc/promtail
#   curl -LO https://github.com/grafana/loki/releases/download/v2.2.1/promtail-linux-amd64.zip
#   apt update
#   apt install unzip
#   unzip promtail-linux-amd64.zip
#   mv promtail-linux-amd64 /usr/bin/promtail
#   rm -r /tmp/$TMP
# EOF

scp $GIT_ROOT/server/instances/loki/promtail.service $TARGET:/etc/systemd/system/
scp $GIT_ROOT/server/instances/loki/promtail.yml $TARGET:/etc/promtail/

ssh $TARGET << EOF
  sed -i 's/<HOSTNAME>/$2/g' /etc/promtail/promtail.yml
  systemctl daemon-reload
  systemctl enable promtail
  systemctl restart promtail
  systemctl status promtail
EOF