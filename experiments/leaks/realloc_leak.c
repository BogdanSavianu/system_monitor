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
    if (argc != 6 && argc != 7) {
        fprintf(stderr, "usage: %s <base_kb> <growth_kb_per_step> <jump_every_steps> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t base_kb = parse_u64(argv[1], "base_kb");
    uint64_t growth_kb = parse_u64(argv[2], "growth_kb_per_step");
    uint64_t jump_every = parse_u64(argv[3], "jump_every_steps");
    uint64_t interval_s = parse_u64(argv[4], "interval_s");
    uint64_t steps = parse_u64(argv[5], "steps_0_forever");

    if (base_kb == 0 || jump_every == 0) {
        fprintf(stderr, "base_kb and jump_every_steps must be > 0\n");
        return 1;
    }

    const char *scenario = "realloc_leak";
    const int label = 1;
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

    uint64_t step = 0;
    uint64_t leaked_total_kb = 0;

    uint64_t current_kb = base_kb;
    size_t current_bytes = (size_t)(current_kb * 1024ULL);
    char *active = malloc(current_bytes);
    if (!active) {
        fprintf(stderr, "failed to allocate initial buffer\n");
        return 2;
    }
    memset(active, 0x3C, current_bytes);

    while (steps == 0 || step < steps) {
        uint64_t target_kb = base_kb + (step * growth_kb);
        if (target_kb < current_kb) {
            target_kb = current_kb;
        }

        if ((step % jump_every) == 0 && step != 0) {
            target_kb += base_kb;
        }

        size_t target_bytes = (size_t)(target_kb * 1024ULL);
        char *next = malloc(target_bytes);
        if (!next) {
            fprintf(stderr, "malloc failed at step=%llu\n", (unsigned long long)step);
            return 2;
        }

        size_t to_copy = current_bytes < target_bytes ? current_bytes : target_bytes;
        memcpy(next, active, to_copy);
        memset(next + to_copy, 0x6D, target_bytes - to_copy);

        uint64_t leaked_this_step_kb = target_kb - current_kb;
        leaked_total_kb += leaked_this_step_kb;

        /* Intentionally forget old pointer to mimic leak-prone growth pattern. */
        active = next;
        current_kb = target_kb;
        current_bytes = target_bytes;

        if ((step % 5) == 0) {
            printf("step=%llu leaked_kb_step=%llu leaked_kb_total=%llu workload_kb_this_step=%llu\n",
                   (unsigned long long)step,
                   (unsigned long long)leaked_this_step_kb,
                   (unsigned long long)leaked_total_kb,
                   (unsigned long long)target_kb);
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
                    (unsigned long long)target_kb);
            fflush(csv);
        }

        step++;
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
                fprintf(meta, "base_kb=%llu\n", (unsigned long long)base_kb);
                fprintf(meta, "growth_kb_per_step=%llu\n", (unsigned long long)growth_kb);
                fprintf(meta, "jump_every_steps=%llu\n", (unsigned long long)jump_every);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
