#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "run_id.h"

static uint64_t parse_u64(const char *s, const char *name) {
    unsigned long long v = 0;
    char tail = '\0';
    if (sscanf(s, "%llu%c", &v, &tail) != 1) {
        fprintf(stderr, "invalid %s: %s\n", name, s);
        exit(1);
    }
    return (uint64_t)v;
}

int main(int argc, char **argv) {
    if (argc != 7 && argc != 8) {
        fprintf(stderr, "usage: %s <allocs_per_step> <min_kb> <max_kb> <leak_percent_1_100> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t allocs_per_step = parse_u64(argv[1], "allocs_per_step");
    uint64_t min_kb = parse_u64(argv[2], "min_kb");
    uint64_t max_kb = parse_u64(argv[3], "max_kb");
    uint64_t leak_percent = parse_u64(argv[4], "leak_percent_1_100");
    uint64_t interval_s = parse_u64(argv[5], "interval_s");
    uint64_t steps = parse_u64(argv[6], "steps_0_forever");

    if (allocs_per_step == 0 || min_kb == 0 || max_kb < min_kb || leak_percent == 0 || leak_percent > 100) {
        fprintf(stderr, "invalid arguments\n");
        return 1;
    }

    const char *scenario = "arena_churn_leak";
    const int label = 1;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id = make_run_id();

    FILE *csv = NULL;
    if (argc == 8) {
        csv = fopen(argv[7], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[7]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    srand((unsigned int)time(NULL));

    uint64_t step = 0;
    uint64_t leaked_total_kb = 0;
    uint64_t leaked_live_capacity = 4096;
    uint64_t leaked_live_count = 0;
    void **leaked_live = calloc((size_t)leaked_live_capacity, sizeof(void *));
    if (!leaked_live) {
        fprintf(stderr, "failed to allocate leaked_live\n");
        return 2;
    }

    while (steps == 0 || step < steps) {
        void **batch = calloc((size_t)allocs_per_step, sizeof(void *));
        uint64_t *batch_kb = calloc((size_t)allocs_per_step, sizeof(uint64_t));
        if (!batch || !batch_kb) {
            fprintf(stderr, "allocation failed at step=%llu\n", (unsigned long long)step);
            free(batch);
            free(batch_kb);
            return 2;
        }

        uint64_t workload_kb = 0;
        for (uint64_t i = 0; i < allocs_per_step; i++) {
            uint64_t kb = min_kb + (uint64_t)(rand() % (int)(max_kb - min_kb + 1));
            size_t bytes = (size_t)(kb * 1024ULL);
            void *p = malloc(bytes);
            if (!p) {
                fprintf(stderr, "malloc failed at step=%llu idx=%llu\n",
                        (unsigned long long)step,
                        (unsigned long long)i);
                for (uint64_t j = 0; j < i; j++) {
                    free(batch[j]);
                }
                free(batch);
                free(batch_kb);
                return 2;
            }
            memset(p, 0xB4, bytes);
            batch[i] = p;
            batch_kb[i] = kb;
            workload_kb += kb;
        }

        uint64_t leaked_this_step_kb = 0;
        for (uint64_t i = 0; i < allocs_per_step; i++) {
            uint64_t r = (uint64_t)(rand() % 100) + 1;
            if (r <= leak_percent) {
                if (leaked_live_count == leaked_live_capacity) {
                    uint64_t next_capacity = leaked_live_capacity * 2;
                    void **next = realloc(leaked_live, (size_t)next_capacity * sizeof(void *));
                    if (!next) {
                        fprintf(stderr, "realloc failed while tracking leaked pointers\n");
                        return 2;
                    }
                    leaked_live = next;
                    leaked_live_capacity = next_capacity;
                }
                leaked_live[leaked_live_count++] = batch[i];
                leaked_this_step_kb += batch_kb[i];
            } else {
                free(batch[i]);
            }
        }

        leaked_total_kb += leaked_this_step_kb;

        /* Periodically free older leaked blocks to produce churn with upward drift. */
        if (leaked_live_count > 1024 && (step % 17) == 0) {
            uint64_t to_free = leaked_live_count / 5;
            for (uint64_t i = 0; i < to_free; i++) {
                free(leaked_live[i]);
            }
            memmove(leaked_live, leaked_live + to_free, (size_t)(leaked_live_count - to_free) * sizeof(void *));
            leaked_live_count -= to_free;
        }

        if ((step % 5) == 0) {
            printf("step=%llu leaked_kb_step=%llu leaked_kb_total=%llu workload_kb_this_step=%llu\n",
                   (unsigned long long)step,
                   (unsigned long long)leaked_this_step_kb,
                   (unsigned long long)leaked_total_kb,
                   (unsigned long long)workload_kb);
            fflush(stdout);
        }

        if (csv) {
            fprintf(csv, "%s,%d,%llu,%llu,%llu,%llu,%llu,%llu\n",
                    scenario,
                    label,
                    run_id,
                    (unsigned long long)step,
                    (unsigned long long)(step * interval_s),
                    (unsigned long long)leaked_this_step_kb,
                    (unsigned long long)leaked_total_kb,
                    (unsigned long long)workload_kb);
            fflush(csv);
        }

        free(batch);
        free(batch_kb);

        step++;
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
                fprintf(meta, "allocs_per_step=%llu\n", (unsigned long long)allocs_per_step);
                fprintf(meta, "min_kb=%llu\n", (unsigned long long)min_kb);
                fprintf(meta, "max_kb=%llu\n", (unsigned long long)max_kb);
                fprintf(meta, "leak_percent=%llu\n", (unsigned long long)leak_percent);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
