VERSION=$(shell grep "^[^ ]" Changes | head -1 | cut -f1 -d' ')
BINARY=target/release/safe-rm
BUILDDIR=safe-rm-$(VERSION)
TARBALL=safe-rm-$(VERSION).tar.gz

$(BINARY):
	cargo build --release

dist: $(TARBALL)
	gpg --armor --sign --detach-sig $(TARBALL)

$(TARBALL): $(BINARY)
	mkdir $(BUILDDIR)
	cp `cat Manifest` $(BUILDDIR)
	tar zcf $(TARBALL) $(BUILDDIR)
	rm -rf $(BUILDDIR)

clean:
	-rm -rf $(TARBALL) $(TARBALL).asc $(BUILDDIR) target

test:
	cargo check
	cargo test
