CFLAGS ?= -O0 -g -fno-omit-frame-pointer
RELEASE_CFLAGS ?= -O2 -fno-inline -fno-omit-frame-pointer

all: steady_leak bursty_leak staircase_leak subtle_leak control_workload cpu_spiky_stable_mem noisy_leak arena_churn_leak realloc_leak mmap_sparse_leak calloc_leak multi_alloc_leak multi_alloc_leak_release

run_id.o: run_id.c run_id.h
	cc $(CFLAGS) -c -o run_id.o run_id.c

steady_leak: steady_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o steady_leak steady_leak.c run_id.o

bursty_leak: bursty_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o bursty_leak bursty_leak.c run_id.o

staircase_leak: staircase_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o staircase_leak staircase_leak.c run_id.o

subtle_leak: subtle_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o subtle_leak subtle_leak.c run_id.o

control_workload: control_workload.c run_id.h run_id.o
	cc $(CFLAGS) -o control_workload control_workload.c run_id.o

cpu_spiky_stable_mem: cpu_spiky_stable_mem.c run_id.h run_id.o
	cc $(CFLAGS) -o cpu_spiky_stable_mem cpu_spiky_stable_mem.c run_id.o -lm

noisy_leak: noisy_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o noisy_leak noisy_leak.c run_id.o

arena_churn_leak: arena_churn_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o arena_churn_leak arena_churn_leak.c run_id.o

realloc_leak: realloc_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o realloc_leak realloc_leak.c run_id.o

mmap_sparse_leak: mmap_sparse_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o mmap_sparse_leak mmap_sparse_leak.c run_id.o

calloc_leak: calloc_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o calloc_leak calloc_leak.c run_id.o

multi_alloc_leak: multi_alloc_leak.c run_id.h run_id.o
	cc $(CFLAGS) -o multi_alloc_leak multi_alloc_leak.c run_id.o

# same source, built without debug info: file:line is unavailable, so use the fallback
multi_alloc_leak_release: multi_alloc_leak.c run_id.h run_id.o
	cc $(RELEASE_CFLAGS) -o multi_alloc_leak_release multi_alloc_leak.c run_id.o

debug: clean all

clean:
	rm -f steady_leak bursty_leak staircase_leak subtle_leak control_workload cpu_spiky_stable_mem noisy_leak arena_churn_leak realloc_leak mmap_sparse_leak calloc_leak multi_alloc_leak multi_alloc_leak_release run_id.o

.PHONY: all clean debug
