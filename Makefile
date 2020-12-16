VERSION=$(shell grep "^[^ ]" Changes | head -1 | cut -f1 -d' ')
BINARY=target/release/safe-rm
BUILDDIR=safe-rm-$(VERSION)
TARBALL=safe-rm-$(VERSION).tar.gz

all: $(BINARY)

$(BINARY):
	cargo build --release

dist: $(TARBALL)
	gpg --armor --sign --detach-sig $(TARBALL)

$(TARBALL):
	mkdir $(BUILDDIR)
	cp -r `cat Manifest` $(BUILDDIR)
	tar zcf $(TARBALL) $(BUILDDIR)
	rm -rf $(BUILDDIR)

clean:
	-rm -rf $(TARBALL) $(TARBALL).asc $(BUILDDIR) target

test:
	cargo check --all-targets
	cargo test

lint:
	cargo-geiger --all-dependencies --quiet true
	cargo audit --deny-warnings --quiet
	cargo clippy --quiet
	cargo tarpaulin --fail-under 90
