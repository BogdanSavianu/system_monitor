#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>
#include <unistd.h>

#include "run_id.h"

uint64_t parse_u64(const char *s, const char *name) {
    unsigned long long v = 0;
    char tail = '\0';
    if (sscanf(s, "%llu%c", &v, &tail) != 1) {
        fprintf(stderr, "invalid %s: %s\n", name, s);
        exit(1);
    }
    return (uint64_t)v;
}

uint64_t clamp_u64(uint64_t v, uint64_t low, uint64_t high) {
    if (v < low) {
        return low;
    }
    if (v > high) {
        return high;
    }
    return v;
}

int main(int argc, char **argv) {
    if (argc != 7 && argc != 8) {
        fprintf(stderr, "usage: %s <base_kb> <jitter_pct_0_100> <cpu_spike_pct_0_100> <max_cpu_spike_ms> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t base_kb = parse_u64(argv[1], "base_kb");
    uint64_t jitter_pct = parse_u64(argv[2], "jitter_pct");
    uint64_t cpu_spike_pct = parse_u64(argv[3], "cpu_spike_pct");
    uint64_t max_cpu_spike_ms = parse_u64(argv[4], "max_cpu_spike_ms");
    uint64_t interval_s = parse_u64(argv[5], "interval_s");
    uint64_t steps = parse_u64(argv[6], "steps");
    const char *scenario = "noisy_leak";
    const int label = 1;

    jitter_pct = clamp_u64(jitter_pct, 0, 100);
    cpu_spike_pct = clamp_u64(cpu_spike_pct, 0, 100);

    time_t start_epoch_s = time(NULL);
    unsigned long long run_id = make_run_id();
    srand((unsigned int)run_id);

    FILE *csv = NULL;
    if (argc == 8) {
        csv = fopen(argv[7], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[7]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    uint64_t i = 0;
    uint64_t total_kb = 0;

    while (steps == 0 || i < steps) {
        int direction = (rand() % 3) - 1;
        uint64_t jitter_span = (base_kb * jitter_pct) / 100ULL;
        int64_t delta = (int64_t)direction * (int64_t)(jitter_span / 2ULL);
        int64_t raw = (int64_t)base_kb + delta;
        uint64_t this_kb = (uint64_t)(raw < 1 ? 1 : raw);

        size_t bytes = (size_t)(this_kb * 1024ULL);
        void *p = malloc(bytes);
        if (!p) {
            fprintf(stderr, "malloc failed at step=%llu\n", (unsigned long long)i);
            return 2;
        }
        memset(p, 0xE1, bytes);

        total_kb += this_kb;

        if ((uint64_t)(rand() % 100) < cpu_spike_pct) {
            uint64_t busy_ms = (uint64_t)(rand() % (max_cpu_spike_ms + 1));
            struct timeval start, now;
            gettimeofday(&start, NULL);
            do {
                volatile uint64_t spin = 0;
                for (uint64_t k = 0; k < 200000; k++) {
                    spin += k;
                }
                (void)spin;
                gettimeofday(&now, NULL);
                long elapsed_ms = (now.tv_sec - start.tv_sec) * 1000L +
                                  (now.tv_usec - start.tv_usec) / 1000L;
                if (elapsed_ms >= (long)busy_ms) {
                    break;
                }
            } while (1);
        }

        if ((i % 5) == 0) {
            printf("step=%llu leaked_kb_step=%llu leaked_kb_total=%llu\n",
                   (unsigned long long)i,
                   (unsigned long long)this_kb,
                   (unsigned long long)total_kb);
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                    scenario,
                    label,
                    run_id,
                    (unsigned long long)i,
                    (unsigned long long)(i * interval_s),
                    (unsigned long long)this_kb,
                    (unsigned long long)total_kb,
                    (unsigned long long)this_kb);
            fflush(csv);
        }

        i++;
        sleep((unsigned int)interval_s);
    }

    if (csv) {
        fclose(csv);

        char meta_path[1024];
        if (snprintf(meta_path, sizeof(meta_path), "%s.meta", argv[7]) > 0) {
            FILE *meta = fopen(meta_path, "w");
            if (meta) {
                time_t end_epoch_s = time(NULL);
                fprintf(meta, "scenario=%s\n", scenario);
                fprintf(meta, "label=%d\n", label);
                fprintf(meta, "run_id=%llu\n", run_id);
                fprintf(meta, "start_epoch_s=%lld\n", (long long)start_epoch_s);
                fprintf(meta, "end_epoch_s=%lld\n", (long long)end_epoch_s);
                fprintf(meta, "base_kb=%llu\n", (unsigned long long)base_kb);
                fprintf(meta, "jitter_pct=%llu\n", (unsigned long long)jitter_pct);
                fprintf(meta, "cpu_spike_pct=%llu\n", (unsigned long long)cpu_spike_pct);
                fprintf(meta, "max_cpu_spike_ms=%llu\n", (unsigned long long)max_cpu_spike_ms);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
