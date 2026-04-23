CC ?= cc
CFLAGS ?= -O2 -Wall -Wextra -std=c11

TARGETS = steady_leak bursty_leak staircase_leak subtle_leak control_workload cpu_spiky_stable_mem noisy_leak

all: $(TARGETS)

steady_leak: steady_leak.c
	$(CC) $(CFLAGS) -o $@ $<

bursty_leak: bursty_leak.c
	$(CC) $(CFLAGS) -o $@ $<

staircase_leak: staircase_leak.c
	$(CC) $(CFLAGS) -o $@ $<

subtle_leak: subtle_leak.c
	$(CC) $(CFLAGS) -o $@ $<

control_workload: control_workload.c
	$(CC) $(CFLAGS) -o $@ $<

cpu_spiky_stable_mem: cpu_spiky_stable_mem.c
	$(CC) $(CFLAGS) -o $@ $< -lm

noisy_leak: noisy_leak.c
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(TARGETS)

.PHONY: all clean
