.DEFAULT_GOAL := help

# Convenience targets for Linux; other platforms can use Cargo directly.
SHELL := /bin/sh

CARGO ?= cargo
INSTALL ?= install
CARGO_TARGET_DIR ?= target
export CARGO_TARGET_DIR
# Let Cargo use its standard home unless the caller explicitly overrides it.
ifneq ($(origin CARGO_HOME),undefined)
export CARGO_HOME
endif
INSTALL_UID := $(shell id -u)
BUILD_HOME := $(HOME)
USER_CARGO_HOME = $(if $(strip $(CARGO_HOME)),$(CARGO_HOME),$(BUILD_HOME)/.cargo)
CARGO_COMMAND = $(CARGO)

ifeq ($(INSTALL_UID),0)
PREFIX ?= /usr/local
# A sudo install builds as the original user to reuse their Rust toolchain
# and avoid creating root-owned files in their dependency/build caches.
ifneq ($(strip $(SUDO_USER)),)
ifneq ($(SUDO_USER),root)
BUILD_HOME := $(shell getent passwd "$(SUDO_USER)" | cut -d: -f6)
ifeq ($(strip $(BUILD_HOME)),)
$(error Cannot determine the home directory for SUDO_USER)
endif
CARGO_COMMAND = sudo -H -u "$(SUDO_USER)" env PATH="$(USER_CARGO_HOME)/bin:$(PATH)" $(if $(strip $(CARGO_HOME)),CARGO_HOME="$(CARGO_HOME)") CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" $(CARGO)
endif
endif
else
PREFIX ?= $(HOME)/.local
endif

BINDIR ?= $(PREFIX)/bin
DESTDIR ?=

RELEASE_BIN = $(CARGO_TARGET_DIR)/release/aigate-cli
INSTALLED_BIN = $(DESTDIR)$(BINDIR)/aigate-cli

.PHONY: help all build clean install uninstall fmt fmt-check lint test test-install test-release check

help:
	@printf '%s\n' \
	  'make build                       编译 release 版本' \
	  'make clean                       清理编译产物' \
	  'make install                     编译并安装到 ~/.local/bin' \
	  'sudo make install                编译并全局安装到 /usr/local/bin' \
	  'make uninstall                   卸载当前用户的程序' \
	  'sudo make uninstall              卸载全局程序' \
	  'make check                       运行格式检查、lint 和测试' \
	  'make test-install                验证安装路径与失败行为（临时目录）' \
	  'make test-release                验证源码发行与拒绝条件（临时仓库）' \
	  'make fmt                         格式化 Rust 源码' \
	  '支持 PREFIX、BINDIR、DESTDIR、CARGO_HOME、CARGO_TARGET_DIR 覆盖。'

all: build

build:
	$(CARGO_COMMAND) build --release --locked --bin aigate-cli

clean:
	$(CARGO_COMMAND) clean

install: build
	$(INSTALL) -Dm755 -- "$(RELEASE_BIN)" "$(INSTALLED_BIN)"
	@printf '已安装：%s\n' "$(INSTALLED_BIN)"

uninstall:
	rm -f -- "$(INSTALLED_BIN)"
	@printf '已卸载：%s\n' "$(INSTALLED_BIN)"

fmt:
	$(CARGO_COMMAND) fmt --all

fmt-check:
	$(CARGO_COMMAND) fmt --all -- --check

lint:
	$(CARGO_COMMAND) clippy --locked --all-targets -- -D warnings

test:
	$(CARGO_COMMAND) test --locked --all-targets
	$(MAKE) test-install
	$(MAKE) test-release

test-install:
	sh tests/makefile.sh

test-release:
	bash tests/release.sh
	bash tests/release-signing.sh
	bash tests/nightly.sh

check:
	$(MAKE) fmt-check
	$(MAKE) lint
	$(MAKE) test
