echo "Checking dependency requirements are met: cargo, docker" &&
which docker && which cargo &&
echo "Success\n" && 

echo "Checking if docker engine is running" &&
docker info &&
echo "Success\n" &&

echo "Sourcing secrets for QA" &&
source qa_secrets.sh &&
echo "Success\n" && 

echo "Grabbing the cross compiler" &&
cargo install cross && 
echo "Success\n" &&

echo "Building" &&
cross build --target x86_64-unknown-linux-gnu && 
echo "Success\n" &&

echo "\nKilling any servers currently running" &&
ssh root@lockbook.app 'killall lockbook-server; echo Status: $?' && 

echo "Copying binary" &&
scp target/x86_64-unknown-linux-gnu/debug/lockbook-server root@lockbook.app:~ &&
echo "Success\n" &&

ssh root@lockbook.app '(ROCKET_ENV=prod nohup ./lockbook-server >> lockbook-server.log 2>&1) &'
