OPTIONS = --use-boolean-returns true \
          --use-symbols-in-builtins true \
          --custom-errors true \
          --use-uuobjids true \
          --anonymous-objects false

DURATION ?= 30
SUBSCRIBERS ?= 256
WORK_PER_TICK ?= 20
APPEND_MODE ?= 0
SLOW_DB_PATH ?= /tmp/moor-slow-mount/bench.db

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

bench-write-stress: $(wildcard src/*.moo)
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
	--test-wizard=2 --run-tests true --test-filter "#668:test_write_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(APPEND_MODE)}"

# Profile with perf to identify blocking spots
# After running, analyze with: perf report -g or perf report --stdio
bench-write-stress-perf: $(wildcard src/*.moo)
	cargo build --release -p moorc
	perf record -g --call-graph dwarf -o perf.data -- \
		../../target/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
		--test-wizard=2 --run-tests true --test-filter "#668:test_write_stress" --test-timeout 60
	@echo "Perf data written to perf.data"
	@echo "Analyze with: perf report -g"

# Profile with scheduler tracing to see blocking/sleeping (requires root or perf_event_paranoid=-1)
# Build first without sudo, then run perf with sudo
# Also captures iostat for I/O analysis
bench-write-stress-perf-sched: $(wildcard src/*.moo)
	cargo build --release -p moorc
	@echo "Running perf with sudo (will prompt for password)..."
	@echo "Starting iostat logging to iostat.log..."
	@rm -f iostat.log && \
	iostat -x 1 > iostat.log & \
	IOSTAT_PID=$$! && \
	sudo perf record -g --call-graph dwarf -e sched:sched_switch,sched:sched_stat_sleep -o perf-sched.data -- \
		../../target/release/moorc $(OPTIONS) \
		--src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
		--test-wizard=2 --run-tests true --test-filter "#668:test_write_stress" --test-timeout 60 ; \
	kill $$IOSTAT_PID 2>/dev/null || true
	sudo chown $(USER):$(USER) perf-sched.data
	@echo "Perf data written to perf-sched.data"
	@echo "I/O stats written to iostat.log"
	@echo "Analyze with: perf report -i perf-sched.data -g"

# Run benchmark on simulated slow storage (requires setup-slow-disk.sh first)
# Usage: make bench-write-stress-slow SLOW_DB_PATH=/tmp/moor-slow-mount/bench.db DURATION=300
bench-write-stress-slow: $(wildcard src/*.moo)
	@if [ ! -d "$$(dirname $(SLOW_DB_PATH))" ]; then \
		echo "Error: Slow disk not mounted. Run: ../../scripts/setup-slow-disk.sh setup"; \
		exit 1; \
	fi
	@echo "Starting iostat logging to iostat-slow.log..."
	@rm -f iostat-slow.log && \
	iostat -x 1 > iostat-slow.log & \
	IOSTAT_PID=$$! && \
	$(MOORC) --src-objdef-dir $(SRC_DIRECTORY) --out-objdef-dir $(OUTPUT_DIRECTORY)/gen.objdir \
	--db-path $(SLOW_DB_PATH) \
	--test-wizard=2 --run-tests true --test-filter "#668:test_write_stress" --test-timeout $$(($(DURATION) + 60)) --test-args "{$(DURATION), $(SUBSCRIBERS), $(WORK_PER_TICK), $(APPEND_MODE)}" ; \
	kill $$IOSTAT_PID 2>/dev/null || true
	@echo "I/O stats written to iostat-slow.log"

clean:
	rm -rf gen.objdir
