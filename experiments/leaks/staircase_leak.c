#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
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
    if (argc != 5 && argc != 6) {
        fprintf(stderr, "usage: %s <start_kb> <increment_kb> <interval_s> <steps_0_forever> [output_csv]\n", argv[0]);
        return 1;
    }

    uint64_t start_kb = parse_u64(argv[1], "start_kb");
    uint64_t increment_kb = parse_u64(argv[2], "increment_kb");
    uint64_t interval_s = parse_u64(argv[3], "interval_s");
    uint64_t steps = parse_u64(argv[4], "steps");
    const char *scenario = "staircase_leak";
    const int label = 1;
    time_t start_epoch_s = time(NULL);
    unsigned long long run_id =
        (unsigned long long)start_epoch_s ^ (unsigned long long)getpid();

    FILE *csv = NULL;
    if (argc == 6) {
        csv = fopen(argv[5], "w");
        if (!csv) {
            fprintf(stderr, "failed to open csv file: %s\n", argv[5]);
            return 1;
        }
        fprintf(csv, "scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step\n");
    }

    uint64_t i = 0;
    uint64_t total_kb = 0;

    while (steps == 0 || i < steps) {
        uint64_t this_kb = start_kb + i * increment_kb;
        size_t bytes = (size_t)(this_kb * 1024ULL);

        void *p = malloc(bytes);
        if (!p) {
            fprintf(stderr, "malloc failed at step=%llu\n", (unsigned long long)i);
            return 2;
        }
        memset(p, 0x3C, bytes);

        total_kb += this_kb;
        printf("step=%llu leaked_kb_step=%llu leaked_kb_total=%llu\n",
               (unsigned long long)i,
               (unsigned long long)this_kb,
               (unsigned long long)total_kb);
        fflush(stdout);

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
        if (snprintf(meta_path, sizeof(meta_path), "%s.meta", argv[5]) > 0) {
            FILE *meta = fopen(meta_path, "w");
            if (meta) {
                time_t end_epoch_s = time(NULL);
                fprintf(meta, "scenario=%s\n", scenario);
                fprintf(meta, "label=%d\n", label);
                fprintf(meta, "run_id=%llu\n", run_id);
                fprintf(meta, "start_epoch_s=%lld\n", (long long)start_epoch_s);
                fprintf(meta, "end_epoch_s=%lld\n", (long long)end_epoch_s);
                fprintf(meta, "start_kb=%llu\n", (unsigned long long)start_kb);
                fprintf(meta, "increment_kb=%llu\n", (unsigned long long)increment_kb);
                fprintf(meta, "interval_s=%llu\n", (unsigned long long)interval_s);
                fprintf(meta, "steps=%llu\n", (unsigned long long)steps);
                fclose(meta);
            }
        }
    }

    return 0;
}
