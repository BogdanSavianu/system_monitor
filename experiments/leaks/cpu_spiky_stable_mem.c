#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>
#include <unistd.h>

uint64_t parse_u64(const char *s, const char *name) {
    unsigned long long v = 0;
    char tail = '\0';
    if (sscanf(s, "%llu%c", &v, &tail) != 1) {
        fprintf(stderr, "invalid %s: %s\n", name, s);
        exit(1);
    }
    return (uint64_t)v;
}

int main(int argc, char **argv) {
    if (argc != 6 && argc != 7) {
        fprintf(stderr, "usage: %s <base_kb> <cpu_spike_every_steps> <cpu_spike_ms> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t base_kb = parse_u64(argv[1], "base_kb");
    uint64_t spike_every = parse_u64(argv[2], "cpu_spike_every_steps");
    uint64_t spike_ms = parse_u64(argv[3], "cpu_spike_ms");
    uint64_t interval_s = parse_u64(argv[4], "interval_s");
    uint64_t steps = parse_u64(argv[5], "steps");
    const char *scenario = "cpu_spiky_stable_mem";
    const int label = 0;

    if (spike_every == 0) {
        fprintf(stderr, "cpu_spike_every_steps must be > 0\n");
        return 1;
    }

    size_t bytes = (size_t)(base_kb * 1024ULL);
    void *stable = malloc(bytes);
    if (!stable) {
        fprintf(stderr, "malloc failed for base buffer\n");
        return 2;
    }
    memset(stable, 0x11, bytes);

    time_t start_epoch_s = time(NULL);
    unsigned long long run_id =
        (unsigned long long)start_epoch_s ^ (unsigned long long)getpid();

    FILE *csv = NULL;
    if (argc == 7) {
        csv = fopen(argv[6], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[6]);
            free(stable);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    uint64_t i = 0;
    while (steps == 0 || i < steps) {
        uint64_t workload_kb = base_kb;

        if ((i % spike_every) == 0) {
            struct timeval start, now;
            gettimeofday(&start, NULL);
            do {
                volatile double x = 0.0;
                for (int k = 0; k < 20000; k++) {
                    x += sin((double)k);
                }
                (void)x;
                gettimeofday(&now, NULL);
                long elapsed_ms = (now.tv_sec - start.tv_sec) * 1000L +
                                  (now.tv_usec - start.tv_usec) / 1000L;
                if (elapsed_ms >= (long)spike_ms) {
                    break;
                }
            } while (1);
        }

        if ((i % 5) == 0) {
            printf("step=%llu stable_kb=%llu\n",
                   (unsigned long long)i,
                   (unsigned long long)base_kb);
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,0,0,%llu\n",
                    scenario,
                    label,
                    run_id,
                    (unsigned long long)i,
                    (unsigned long long)(i * interval_s),
                    (unsigned long long)workload_kb);
            fflush(csv);
        }

        i++;
        sleep((unsigned int)interval_s);
    }

    free(stable);

    if (csv) {
        fclose(csv);

        char meta_path[1024];
        if (snprintf(meta_path, sizeof(meta_path), "%s.meta", argv[6]) > 0) {
            FILE *meta = fopen(meta_path, "w");
            if (meta) {
                time_t end_epoch_s = time(NULL);
                fprintf(meta, "scenario=%s\n", scenario);
                fprintf(meta, "label=%d\n", label);
                fprintf(meta, "run_id=%llu\n", run_id);
                fprintf(meta, "start_epoch_s=%lld\n", (long long)start_epoch_s);
                fprintf(meta, "end_epoch_s=%lld\n", (long long)end_epoch_s);
                fprintf(meta, "base_kb=%llu\n", (unsigned long long)base_kb);
                fprintf(meta, "cpu_spike_every_steps=%llu\n", (unsigned long long)spike_every);
                fprintf(meta, "cpu_spike_ms=%llu\n", (unsigned long long)spike_ms);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
