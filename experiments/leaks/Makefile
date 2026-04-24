all: steady_leak bursty_leak staircase_leak subtle_leak control_workload cpu_spiky_stable_mem noisy_leak

run_id.o: run_id.c run_id.h
	cc -c -o run_id.o run_id.c

steady_leak: steady_leak.c run_id.h run_id.o
	cc -o steady_leak steady_leak.c run_id.o

bursty_leak: bursty_leak.c run_id.h run_id.o
	cc -o bursty_leak bursty_leak.c run_id.o

staircase_leak: staircase_leak.c run_id.h run_id.o
	cc -o staircase_leak staircase_leak.c run_id.o

subtle_leak: subtle_leak.c run_id.h run_id.o
	cc -o subtle_leak subtle_leak.c run_id.o

control_workload: control_workload.c run_id.h run_id.o
	cc -o control_workload control_workload.c run_id.o

cpu_spiky_stable_mem: cpu_spiky_stable_mem.c run_id.h run_id.o
	cc -o cpu_spiky_stable_mem cpu_spiky_stable_mem.c run_id.o -lm

noisy_leak: noisy_leak.c run_id.h run_id.o
	cc -o noisy_leak noisy_leak.c run_id.o

clean:
	rm -f steady_leak bursty_leak staircase_leak subtle_leak control_workload cpu_spiky_stable_mem noisy_leak run_id.o

.PHONY: all clean
