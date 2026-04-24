#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
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

int main(int argc, char **argv) {
    if (argc != 6 && argc != 7) {
        fprintf(stderr, "usage: %s <allocs_per_step> <kb_per_alloc> <burst_every_steps> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t allocs = parse_u64(argv[1], "allocs_per_step");
    uint64_t kb_per_alloc = parse_u64(argv[2], "kb_per_alloc");
    uint64_t burst_every = parse_u64(argv[3], "burst_every_steps");
    uint64_t interval_s = parse_u64(argv[4], "interval_s");
    uint64_t steps = parse_u64(argv[5], "steps");
    const char *scenario = "control_workload";
    const int label = 0;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id = make_run_id();

    FILE *csv = NULL;
    if (argc == 7) {
        csv = fopen(argv[6], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[6]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    if (allocs == 0) {
        fprintf(stderr, "allocs_per_step must be > 0\n");
        return 1;
    }
    if (burst_every == 0) {
        fprintf(stderr, "burst_every_steps must be > 0\n");
        return 1;
    }

    size_t bytes = (size_t)(kb_per_alloc * 1024ULL);

    uint64_t i = 0;
    while (steps == 0 || i < steps) {
        uint64_t this_allocs = allocs;
        if ((i % burst_every) == 0) {
            this_allocs = allocs * 3;
        }

        void **ptrs = calloc((size_t)this_allocs, sizeof(void *));
        if (!ptrs) {
            fprintf(stderr, "calloc failed at step=%llu\n", (unsigned long long)i);
            return 2;
        }

        for (uint64_t j = 0; j < this_allocs; j++) {
            ptrs[j] = malloc(bytes);
            if (!ptrs[j]) {
                fprintf(stderr, "malloc failed at step=%llu alloc=%llu\n",
                        (unsigned long long)i,
                        (unsigned long long)j);
                for (uint64_t k = 0; k < j; k++) {
                    free(ptrs[k]);
                }
                free(ptrs);
                return 2;
            }
            memset(ptrs[j], 0x7F, bytes);
        }

        for (uint64_t j = 0; j < this_allocs; j++) {
            free(ptrs[j]);
        }
        free(ptrs);

        if ((i % 5) == 0) {
            printf("step=%llu active_leaked_kb=0 workload_kb_this_step=%llu\n",
                   (unsigned long long)i,
                   (unsigned long long)(this_allocs * kb_per_alloc));
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,0,0,%llu\n",
                scenario,
                label,
                run_id,
                    (unsigned long long)i,
                    (unsigned long long)(i * interval_s),
                    (unsigned long long)(this_allocs * kb_per_alloc));
            fflush(csv);
        }

        i++;
        sleep((unsigned int)interval_s);
    }

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
                fprintf(meta, "allocs_per_step=%llu\n", (unsigned long long)allocs);
                fprintf(meta, "kb_per_alloc=%llu\n", (unsigned long long)kb_per_alloc);
                fprintf(meta, "burst_every_steps=%llu\n", (unsigned long long)burst_every);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
