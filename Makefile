VERSION=$(shell grep "^[^ ]" Changes | head -1 | cut -f1 -d' ')
BUILDDIR=safe-rm-$(VERSION)
TARBALL=safe-rm-$(VERSION).tar.gz

dist: $(TARBALL)

$(TARBALL):
	mkdir $(BUILDDIR)
	cp `cat Manifest` $(BUILDDIR)
	tar zcf safe-rm-$(VERSION).tar.gz $(BUILDDIR)
	rm -rf $(BUILDDIR)

clean:
	-rm -rf $(TARBALL) $(BUILDDIR)
