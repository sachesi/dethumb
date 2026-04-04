OUTDIR ?= $(outdir)
SPEC ?= $(spec)

NAME := $(shell awk '/^Name:[[:space:]]*/ {print $$2; exit}' $(SPEC))
VERSION := $(shell awk '/^Version:[[:space:]]*/ {print $$2; exit}' $(SPEC))
TARBALL := $(NAME)-$(VERSION).tar.gz

.PHONY: srpm clean

srpm:
	@test -n "$(OUTDIR)" && test -n "$(SPEC)"
	mkdir -p "$(OUTDIR)"
	git archive --format=tar.gz \
		--prefix="$(NAME)-$(VERSION)/" \
		-o "$(OUTDIR)/$(TARBALL)" HEAD
	rpmbuild -bs "$(SPEC)" \
		--define "_sourcedir $(OUTDIR)" \
		--define "_srcrpmdir $(OUTDIR)"

clean:
	@test -n "$(OUTDIR)"
	rm -f "$(OUTDIR)/$(TARBALL)" "$(OUTDIR)"/*.src.rpm
