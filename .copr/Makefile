OUTDIR ?= $(outdir)
SPECROOT ?= $(spec)
SPECFILE := $(firstword $(wildcard $(SPECROOT)/*.spec))

NAME := $(shell awk '/^Name:[[:space:]]*/ {print $$2; exit}' $(SPECFILE))
VERSION := $(shell awk '/^Version:[[:space:]]*/ {print $$2; exit}' $(SPECFILE))
SOURCE0 := $(shell awk '/^Source0:[[:space:]]*/ {print $$2; exit}' $(SPECFILE))
TARBALL := $(shell printf '%s' '$(SOURCE0)' | sed -e 's/%{name}/$(NAME)/g' -e 's/%{version}/$(VERSION)/g')

.PHONY: srpm clean

srpm:
	@test -n "$(OUTDIR)"
	@test -n "$(SPECROOT)"
	@test -n "$(SPECFILE)"
	mkdir -p "$(OUTDIR)"
	tar --exclude-vcs --transform="s,^,$(NAME)-$(VERSION)/," -czf "$(OUTDIR)/$(TARBALL)" .
	rpmbuild -bs "$(SPECFILE)" \
	  --define "_sourcedir $(OUTDIR)" \
	  --define "_srcrpmdir $(OUTDIR)"

clean:
	@test -n "$(OUTDIR)"
	rm -f "$(OUTDIR)/$(TARBALL)" "$(OUTDIR)"/*.src.rpm
