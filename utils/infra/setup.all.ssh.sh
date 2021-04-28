#!/bin/sh

# Preamble
set -a
. "$(dirname $0)"/vars.env

TARGET=$1
ssh -q admin@$TARGET -i $GIT_ROOT/intro.pem exit || { echo "could not SSH into $TARGET"; exit 1; }

echo "Adding key from $2 to $1"

ssh admin@$TARGET -i $GIT_ROOT/intro.pem << EOF
  sudo su
  mkdir -p /root/.ssh/
  curl -L $2 >> /root/.ssh/authorized_keys
  systemctl restart sshd
EOF
