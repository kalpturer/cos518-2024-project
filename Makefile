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

test:
	make clean
	cargo build
	bash test.sh