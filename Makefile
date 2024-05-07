.DEFAULT_GOAL := default

default:
	cargo build

build:
	cargo build

clean:
	cargo clean
	rm -f id_*
	rm -f *.log
	rm -f *.err	

test3:
	rm -f id_*
	rm -f *.log
	rm -f *.err
	cargo build --release
	bash ./test_scripts/test3.sh

test5:
	rm -f id_*
	rm -f *.log
	rm -f *.err
	cargo build --release
	bash ./test_scripts/test5.sh