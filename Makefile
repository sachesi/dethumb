SPEC := dethumb.spec
NAME := $(shell awk '/^Name:[[:space:]]*/ {print $$2; exit}' $(SPEC))
VERSION := $(shell awk '/^Version:[[:space:]]*/ {print $$2; exit}' $(SPEC))
TARBALL := $(NAME)-$(VERSION).tar.gz

.PHONY: srpm clean

srpm: $(TARBALL)
	rpmbuild -bs $(SPEC) \
		--define "_sourcedir $(CURDIR)" \
		--define "_srcrpmdir $(CURDIR)"

$(TARBALL):
	git archive --format=tar.gz \
		--prefix=$(NAME)-$(VERSION)/ \
		-o $@ HEAD

clean:
	rm -f $(TARBALL) *.src.rpm
