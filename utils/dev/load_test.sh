# Make Account
export API_URL=https://api.prod.lockbook.net
export LOCKBOOK_CLI_LOCATION=/tmp/load_test_$(date +%s)
echo $API_URL @ $LOCKBOOK_CLI_LOCATION
mkdir -p $LOCKBOOK_CLI_LOCATION

USERNAME=loadtest001
# Will prompt for stdin
lockbook new-account

# Make a megabyte of junk
base64 /dev/urandom | head -c 1048576 | egrep -ao "\w" | tr -d '\n' > test_1M.txt

# Start pump
lockbook copy test_1M.txt $(lockbook whoami)/
lockbook sync

while true; do lockbook sync > /dev/null; done
