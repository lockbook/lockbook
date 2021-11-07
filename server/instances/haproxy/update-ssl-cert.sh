#!/bin/sh

CERTBOT_SITES_DIR="/etc/letsencrypt/live"
for SITE in $(ls "$CERTBOT_SITES_DIR" ); do
	if [ -d "$CERTBOT_SITES_DIR/$SITE" ]; then
	       	echo "found site $SITE"
		CERTBOT_CERT_DIR="/etc/letsencrypt/live/$SITE"
		SSL_CERT="/etc/ssl/$SITE/$SITE.pem"

		[ -z "$SITE" ] && echo "usage: $0 <your-site>" \
			&& exit 1
		[ ! -d "$CERTBOT_CERT_DIR" ] && echo "certbot cert directory $CERTBOT_CERT_DIR does not exist!" \
			&& exit 1
		[ ! -f "$SSL_CERT" ] && echo "current SSL certificate $SSL_CERT does not exist" \
			&& exit 1

		echo "combining and updating SSL cert for $SITE"
		cat "$CERTBOT_CERT_DIR/fullchain.pem" "$CERTBOT_CERT_DIR/privkey.pem" > "$SSL_CERT"
		echo "SSL cert written to $SSL_CERT"
	fi
done

systemctl restart haproxy && echo "HAProxy restarted"
