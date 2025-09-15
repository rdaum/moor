# MOORC binary selection via environment variable MOORC_TYPE
# Options: cargo (default), direct, docker
MOORC_TYPE ?= cargo

# DEBUG controls whether to run moorc under gdb
# Set DEBUG=1 to enable gdb debugging
DEBUG ?= 0

OPTIONS = --use-boolean-returns true \
          --use-symbols-in-builtins true \
          --custom-errors true \
          --use-uuobjids true

SRC_DIRECTORY = src
TEST_DIRECTORY = tests
OUTPUT_DIRECTORY = .
ifeq ($(DEBUG),1)
MOORC = gdb --batch --ex "handle SIGUSR1 nostop noprint pass" --ex run --ex bt --ex quit --args cargo run -p moorc -- \
	$(OPTIONS)
else
MOORC = cargo run -p moorc -- $(OPTIONS)
endif

# Target to generate an old-style MOO textdump from the compilation of the
# objdef style sources in the local directory. This is the default target,
# and is intended mainly just to do a validation/compilation pass.
gen.moo-textdump: $(wildcard src/*.moo)
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-textdump $(OUTPUT_DIRECTORY)/$@

# Target to generate a new objdef dump from the compilation of the local
# directory.
gen.objdir: $(wildcard src/*.moo)
	rm -rf gen.objdir
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir

# Builds a new objdef dump and then copies over the local working sources.
# WARNING: this is destructive to local changes you might have -- it *will*
# overwrite them -- and is meant  mainly as the last step before performing a
# git commit.
rebuild: gen.objdir
	cp gen.objdir/*.moo ./src

test:  $(wildcard src/*.moo)
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY)  --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
	--test-directory $(TEST_DIRECTORY) --test-wizard=2 --test-programmer=6 --test-player=4 --run-tests true

clean:
	rm -f gen.moo-textdump
	rm -rf gen.objdir

output: gen.moo-textdump

