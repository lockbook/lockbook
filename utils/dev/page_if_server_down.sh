#!/bin/sh

if [ -z "$STATUS_API_URL" ]
then
	echo 'No target server to test against. Set env var STATUS_API_URL'
	exit 69
fi

if [ -z "$PD_API_KEY" ]
then
	echo 'No pd api key. Set env var PD_API_KEY.'
	exit 69
fi

key="offline-`date '+%m/%d/%y'`"

# In the future we want to replace --fail with --fail-with-body once new curl proliferates enough
curl --fail $STATUS_API_URL || \
curl --fail \
	-X POST \
	--header 'Content-Type: application/json' \
	--header 'Accept: application/vnd.pagerduty+json;version=2' \
	--header 'From: parth@mehrotra.me' \
	--header "Authorization: Token token=$PD_API_KEY" \
	-d '{
		  "incident": {
			"type": "incident",
			"title": "The server is failing health-checks!",
			"service": {
			  "id": "PJV4ZJU",
			  "type": "service_reference",
			  "summary": null,
			  "self": null,
			  "html_url": null
			},
			"urgency": "high",
			"body": {
			  "type": "incident_body",
			  "details": "Github Actions checks prod every 1-5mins, it hits the /get-build-info endpoint. If curls exit code is non-zero this page occurs."
			},
			"incident_key": "'$key'"
		  }
		}' \
	 'https://api.pagerduty.com/incidents'
