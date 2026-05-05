TOPDIR := $(HOME)/rpmbuild
SOURCES_DIR := $(TOPDIR)/SOURCES
SPEC := dethumb.spec
NAME := dethumb
VERSION := $(shell rpmspec -q --qf '%{VERSION}\n' --srpm $(SPEC) | head -n1)

all: srpm

srpm:
	mkdir -p "$(SOURCES_DIR)"
	spectool -g -R $(SPEC)
	rpmbuild -bs $(SPEC)

clean:
	rm -rf vendor .cargo
