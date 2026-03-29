SASS ?= sass
SCSS_SRC := src/app/gui/styles/app.scss
CSS_OUT := src/app/gui/styles/app.css
BIN_NAME := system-monitor
FEATURES := dioxus-gui
DIST_DIR := dist
OS := linux
ARCH_RAW := $(shell uname -m)
ARCH := $(if $(filter x86_64,$(ARCH_RAW)),amd64,$(if $(filter aarch64 arm64,$(ARCH_RAW)),arm64,$(ARCH_RAW)))
ASSET_BASENAME := $(BIN_NAME)-$(OS)-$(ARCH)
TARBALL := $(DIST_DIR)/$(ASSET_BASENAME).tar.gz
CHECKSUM := $(TARBALL).sha256

.PHONY: css watch-css run-demo check-demo check-gui run-gui release-build release-package clean-dist

VERSION ?=

css:
	@command -v $(SASS) >/dev/null 2>&1 || { echo "sass CLI not found. Install dart-sass first."; exit 1; }
	$(SASS) --no-source-map $(SCSS_SRC):$(CSS_OUT)

watch-css:
	@command -v $(SASS) >/dev/null 2>&1 || { echo "sass CLI not found. Install dart-sass first."; exit 1; }
	$(SASS) --watch --no-source-map $(SCSS_SRC):$(CSS_OUT)

check-demo: css
	cargo check --features dioxus-demo

run-demo: css
	cargo run --features dioxus-demo -- --dioxus-demo

check-gui: css
	cargo check --features dioxus-gui

run-gui: css
	cargo run --features dioxus-gui

release-build:
	cargo build --release --locked --features $(FEATURES)

release-package: release-build
	mkdir -p $(DIST_DIR)
	cp target/release/$(BIN_NAME) $(DIST_DIR)/$(BIN_NAME)
	tar -C $(DIST_DIR) -czf $(TARBALL) $(BIN_NAME)
	sha256sum $(TARBALL) > $(CHECKSUM)
	@echo "Created $(TARBALL)"
	@echo "Created $(CHECKSUM)"

clean-dist:
	rm -rf $(DIST_DIR)

release-help:
	@if [ -z "$(VERSION)" ]; then \
		echo "Usage: make release-help VERSION=vX.Y.Z"; \
		exit 1; \
	fi
	@echo "Release steps for $(VERSION):"
	@echo ""
	@echo "1) Ensure clean and up to date:"
	@echo "   git status"
	@echo "   git pull --ff-only"
	@echo ""
	@echo "2) Optional local validation:"
	@echo "   make check-gui"
	@echo "   make release-package"
	@echo ""
	@echo "3) Create annotated tag:"
	@echo "   git tag -a $(VERSION) -m \"Release $(VERSION)\""
	@echo ""
	@echo "4) Push tag to trigger GitHub Release workflow:"
	@echo "   git push origin $(VERSION)"
	@echo ""
	@echo "5) Verify tag exists remotely:"
	@echo "   git ls-remote --tags origin | grep $(VERSION)"
	@echo ""
	@echo "6) Verify workflow run and assets in GitHub UI:"
	@echo "   Actions -> Release workflow"
	@echo "   Releases -> $(VERSION)"
	@echo ""
	@echo "7) Rollback tag if needed:"
	@echo "   git tag -d $(VERSION)"
	@echo "   git push --delete origin $(VERSION)"