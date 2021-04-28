#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q $TARGET exit || { echo "could not SSH into $TARGET"; exit 1; }

if [ "$2" == "-f" ]; then
ssh $TARGET << EOF
  apt update
  apt install -y haproxy
  add-apt-repository -y ppa:certbot/certbot
  apt-get update
  apt-get install -y certbot
  certbot certonly --standalone -d $TARGET \
  --non-interactive --agree-tos --email raayan@lockbook.net \
  --http-01-port=8888

  mkdir -p /etc/ssl/$TARGET
  cat /etc/letsencrypt/live/$TARGET/fullchain.pem \
  /etc/letsencrypt/live/$TARGET/privkey.pem \
  | tee /etc/ssl/$TARGET/$TARGET.pem
EOF
fi

scp $GIT_ROOT/server/instances/haproxy/load-balancer.haproxy.cfg $TARGET:/etc/haproxy/haproxy.cfg

ssh $TARGET << EOF
  systemctl enable haproxy
  systemctl restart haproxy
  systemctl status haproxy
EOF