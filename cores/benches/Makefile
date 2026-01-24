OPTIONS = --use-boolean-returns true \
          --use-symbols-in-builtins true \
          --custom-errors true \
          --use-uuobjids true \
          --anonymous-objects false

SRC_DIRECTORY = src
TEST_DIRECTORY = tests
OUTPUT_DIRECTORY = .
MOORC = cargo run --release -p moorc -- $(OPTIONS)

bench: $(wildcard src/*.moo)
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
	--test-directory $(TEST_DIRECTORY) --test-wizard=2 --test-programmer=2 --test-player=4 --run-tests true

bench-game-update: $(wildcard src/*.moo)
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
	--test-wizard=2 --run-tests true --test-filter "#668:test_run_bench"

clean:
	rm -rf gen.objdir
