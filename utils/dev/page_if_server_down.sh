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

curl $STATUS_API_URL || \
curl --fail \
	-X POST \
	--header 'Content-Type: application/json' \
	--header 'Accept: application/vnd.pagerduty+json;version=2' \
	--header 'From: parth@mehrotra.me' \
	--header "Authorization: Token token=$PD_API_KEY" \
	-d '{
		  "incident": {
			"type": "incident",
			"title": "string",
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
			  "details": "string"
			},
			"incident_key": "string"
		  }
		}' \
	 'https://api.pagerduty.com/incidents'
