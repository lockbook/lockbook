if cargo fmt --quiet -- --check ; then
	cargo test -- --nocapture
else
	echo "The following files fail lint checks:"
	cargo fmt -- --check -l
	echo "Run cargo fmt" 
	echo "Then re-run this script"
	exit 1
fi
